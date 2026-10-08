//! Field queries.

use super::{Db, Field, FieldId, Kind};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, Row, params};

impl Db {
    pub fn fields(&self) -> Result<Vec<Field>> {
        let fields = self
            .conn
            .prepare("SELECT id, name, kind, position, archived FROM field ORDER BY position")?
            .query_map([], field_from_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(fields)
    }

    pub fn insert_field(&self, name: &str, kind: Kind) -> Result<FieldId> {
        let name = checked_name(&self.conn, name, None)?;
        self.conn.execute(
            "INSERT INTO field (name, kind, position)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position), -1) + 1 FROM field))",
            params![name, kind.as_str()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_field(&self, id: FieldId, name: &str, kind: Kind) -> Result<()> {
        let name = checked_name(&self.conn, name, Some(id))?;
        self.conn.execute(
            "UPDATE field SET name = ?1, kind = ?2 WHERE id = ?3",
            params![name, kind.as_str(), id],
        )?;
        Ok(())
    }

    pub fn set_archived(&self, id: FieldId, archived: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE field SET archived = ?1 WHERE id = ?2",
            params![archived, id],
        )?;
        Ok(())
    }

    /// Swap `id` with the field above (`up`) or below it, then renumber every
    /// position `0..n` so gaps left by deletions close.
    pub fn move_field(&self, id: FieldId, up: bool) -> Result<()> {
        self.transaction(|conn| {
            let mut ids: Vec<FieldId> = conn
                .prepare("SELECT id FROM field ORDER BY position")?
                .query_map([], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            let at = ids.iter().position(|&i| i == id).context("no such field")?;
            let to = if up {
                at.checked_sub(1)
            } else {
                (at + 1 < ids.len()).then_some(at + 1)
            };
            if let Some(to) = to {
                ids.swap(at, to);
            }
            for (position, id) in ids.iter().enumerate() {
                conn.execute(
                    "UPDATE field SET position = ?1 WHERE id = ?2",
                    params![position as i64, id],
                )?;
            }
            Ok(())
        })
    }

    /// Refuse to delete a field any paycheck has an amount for. The schema
    /// refuses too (`amount.field_id` has no cascade); this is the refusal
    /// with a sentence in it.
    pub fn ensure_deletable(&self, id: FieldId) -> Result<()> {
        let (name, used): (String, bool) = self.conn.query_row(
            "SELECT name, EXISTS(SELECT 1 FROM amount WHERE field_id = field.id)
             FROM field WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        ensure!(
            !used,
            "{name} is used by a paycheck; archive it with x instead"
        );
        Ok(())
    }

    pub fn delete_field(&self, id: FieldId) -> Result<()> {
        self.ensure_deletable(id)?;
        self.conn.execute("DELETE FROM field WHERE id = ?1", [id])?;
        Ok(())
    }
}

/// `name` trimmed, or an error if it is blank or another field has it.
fn checked_name(conn: &Connection, name: &str, except: Option<FieldId>) -> Result<String> {
    let name = name.trim();
    ensure!(!name.is_empty(), "a field needs a name");
    let taken: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM field WHERE name = ?1 AND id IS NOT ?2)",
        params![name, except],
        |r| r.get(0),
    )?;
    ensure!(!taken, "a field named {name:?} already exists");
    Ok(name.to_string())
}

fn field_from_row(row: &Row) -> rusqlite::Result<Field> {
    let kind: Kind = row
        .get::<_, String>(2)?
        .parse()
        .map_err(|e: anyhow::Error| {
            rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, e.into())
        })?;
    Ok(Field {
        id: row.get(0)?,
        name: row.get(1)?,
        kind,
        position: row.get(3)?,
        archived: row.get(4)?,
    })
}

#[cfg(test)]
mod tests {
    use crate::db::{Kind, open_in_memory};

    fn names(db: &crate::db::Db) -> Vec<String> {
        db.fields().unwrap().into_iter().map(|f| f.name).collect()
    }

    #[test]
    fn a_new_database_is_seeded_with_the_default_fields_in_order() {
        let db = open_in_memory().unwrap();
        let fields = db.fields().unwrap();
        assert_eq!(
            names(&db),
            [
                "Salary",
                "Federal Tax",
                "Social Security",
                "Medicare",
                "State Tax",
                "Family Leave",
                "Medical Leave",
                "HSA",
                "401K (Trad)",
                "401K (Roth)",
            ]
        );
        assert_eq!(fields[0].kind, Kind::Income);
        assert!(fields[1..].iter().all(|f| f.kind == Kind::Deduction));
        assert!(fields.iter().all(|f| !f.archived));
    }

    #[test]
    fn a_new_field_goes_at_the_end_with_its_name_trimmed() {
        let db = open_in_memory().unwrap();
        let id = db.insert_field("  Bonus ", Kind::Income).unwrap();
        let last = db.fields().unwrap().pop().unwrap();
        assert_eq!(
            (last.id, last.name.as_str(), last.kind),
            (id, "Bonus", Kind::Income)
        );
        assert_eq!(last.position, 10);
    }

    #[test]
    fn a_blank_name_is_refused() {
        let db = open_in_memory().unwrap();
        let err = db.insert_field("   ", Kind::Deduction).unwrap_err();
        assert_eq!(err.to_string(), "a field needs a name");
    }

    #[test]
    fn a_duplicate_name_is_refused() {
        let db = open_in_memory().unwrap();
        let err = db.insert_field("Medicare ", Kind::Deduction).unwrap_err();
        assert_eq!(err.to_string(), "a field named \"Medicare\" already exists");
        let hsa = db.field_id("HSA");
        assert!(db.update_field(hsa, "Medicare", Kind::Deduction).is_err());
    }

    #[test]
    fn a_field_keeps_its_own_name_when_only_its_kind_changes() {
        let db = open_in_memory().unwrap();
        let hsa = db.field_id("HSA");
        db.update_field(hsa, "HSA", Kind::Income).unwrap();
        let field = db
            .fields()
            .unwrap()
            .into_iter()
            .find(|f| f.id == hsa)
            .unwrap();
        assert_eq!(field.kind, Kind::Income);
    }

    #[test]
    fn renaming_a_field_keeps_its_id_and_position() {
        let db = open_in_memory().unwrap();
        let hsa = db.field_id("HSA");
        db.update_field(hsa, "Health Savings", Kind::Deduction)
            .unwrap();
        assert_eq!(db.field_id("Health Savings"), hsa);
        assert_eq!(names(&db)[7], "Health Savings");
    }

    #[test]
    fn archiving_and_unarchiving_a_field() {
        let db = open_in_memory().unwrap();
        let hsa = db.field_id("HSA");
        db.set_archived(hsa, true).unwrap();
        assert!(db.fields().unwrap()[7].archived);
        db.set_archived(hsa, false).unwrap();
        assert!(!db.fields().unwrap()[7].archived);
    }

    #[test]
    fn moving_a_field_swaps_it_with_its_neighbor_and_renumbers_densely() {
        let db = open_in_memory().unwrap();
        db.delete_field(db.field_id("Social Security")).unwrap();
        db.move_field(db.field_id("Medicare"), true).unwrap();
        let fields = db.fields().unwrap();
        assert_eq!(fields[1].name, "Medicare");
        assert_eq!(fields[2].name, "Federal Tax");
        let positions: Vec<i64> = fields.iter().map(|f| f.position).collect();
        assert_eq!(positions, (0..9).collect::<Vec<_>>());
    }

    #[test]
    fn moving_past_either_end_changes_nothing() {
        let db = open_in_memory().unwrap();
        let before = names(&db);
        db.move_field(db.field_id("Salary"), true).unwrap();
        db.move_field(db.field_id("401K (Roth)"), false).unwrap();
        assert_eq!(names(&db), before);
    }

    #[test]
    fn an_unused_field_can_be_deleted() {
        let db = open_in_memory().unwrap();
        db.delete_field(db.field_id("HSA")).unwrap();
        assert!(!names(&db).contains(&"HSA".to_string()));
    }
}
