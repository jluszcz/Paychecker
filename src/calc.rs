//! The Sheet's arithmetic: per-paycheck and YTD totals, net, percentages, and
//! which fields get a row. Pure: it takes fields and paychecks as plain values.

use crate::db::{Field, FieldId, Kind, Paycheck, PaycheckId};
use crate::money::Cents;
use chrono::{Datelike, NaiveDate};
use std::fmt;

/// A percentage in hundredths of a percent: `Percent(2738)` is `27.38%`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Percent(pub i64);

impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.unsigned_abs();
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{sign}{}.{:02}%", abs / 100, abs % 100)
    }
}

/// `part` as a percentage of `income`, rounded half away from zero, or `None`
/// when there is no income to divide by. `i128` because `part * 10_000`
/// overflows `i64` for amounts a `Cents` can hold.
pub fn percent(part: Cents, income: Cents) -> Option<Percent> {
    if income.0 == 0 {
        return None;
    }
    let num = i128::from(part.0) * 10_000;
    let den = i128::from(income.0);
    let (q, r) = (num / den, num % den);
    let q = if r.abs() * 2 >= den.abs() {
        q + num.signum() * den.signum()
    } else {
        q
    };
    Some(Percent(q as i64))
}

pub fn show(percent: Option<Percent>) -> String {
    percent.map_or_else(|| "—".to_string(), |p| p.to_string())
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    pub income: Cents,
    pub net: Cents,
}

pub fn totals(amounts: impl IntoIterator<Item = (Kind, Cents)>) -> Totals {
    let mut t = Totals::default();
    for (kind, cents) in amounts {
        match kind {
            Kind::Income => {
                t.income += cents;
                t.net += cents;
            }
            Kind::Deduction => t.net = t.net - cents,
        }
    }
    t
}

/// Sheet row order, which is also the paycheck form's Tab order.
pub fn sort_rows(fields: &mut [&Field]) {
    fields.sort_by_key(|f| (f.kind == Kind::Deduction, f.position));
}

/// The fields that get a row: every active field, plus any archived field one
/// of `paychecks` has an amount for.
pub fn visible_fields<'a>(fields: &'a [Field], paychecks: &[&Paycheck]) -> Vec<&'a Field> {
    let mut shown: Vec<&Field> = fields
        .iter()
        .filter(|f| !f.archived || paychecks.iter().any(|p| p.amounts.contains_key(&f.id)))
        .collect();
    sort_rows(&mut shown);
    shown
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Column {
    pub id: PaycheckId,
    pub date: NaiveDate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmountRow {
    pub name: String,
    /// `None` where the paycheck has no amount for this field.
    pub cells: Vec<Option<Cents>>,
    pub ytd: Cents,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PercentRow {
    pub label: String,
    pub cells: Vec<Option<Percent>>,
    pub ytd: Option<Percent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sheet {
    pub year: i32,
    pub columns: Vec<Column>,
    pub rows: Vec<AmountRow>,
    pub net: Vec<Cents>,
    pub net_ytd: Cents,
    /// Every visible deduction, then `Net Pay`.
    pub percent_rows: Vec<PercentRow>,
}

/// Lay out `year`: one column per paycheck dated in it, oldest first.
pub fn sheet(year: i32, fields: &[Field], paychecks: &[Paycheck]) -> Sheet {
    let mut checks: Vec<&Paycheck> = paychecks.iter().filter(|p| p.date.year() == year).collect();
    checks.sort_by_key(|p| p.date);
    let kind_of = |id: FieldId| fields.iter().find(|f| f.id == id).map(|f| f.kind);
    let column_totals: Vec<Totals> = checks
        .iter()
        .map(|p| {
            totals(
                p.amounts
                    .iter()
                    .filter_map(|(&id, &c)| Some((kind_of(id)?, c))),
            )
        })
        .collect();
    let ytd = Totals {
        income: column_totals.iter().map(|t| t.income).sum(),
        net: column_totals.iter().map(|t| t.net).sum(),
    };
    let shown = visible_fields(fields, &checks);
    let rows: Vec<AmountRow> = shown
        .iter()
        .map(|f| {
            let cells: Vec<Option<Cents>> = checks
                .iter()
                .map(|p| p.amounts.get(&f.id).copied())
                .collect();
            AmountRow {
                name: f.name.clone(),
                ytd: cells.iter().flatten().copied().sum(),
                cells,
            }
        })
        .collect();
    let mut percent_rows: Vec<PercentRow> = shown
        .iter()
        .zip(&rows)
        .filter(|(f, _)| f.kind == Kind::Deduction)
        .map(|(_, row)| PercentRow {
            label: row.name.clone(),
            cells: row
                .cells
                .iter()
                .zip(&column_totals)
                .map(|(c, t)| percent(c.unwrap_or_default(), t.income))
                .collect(),
            ytd: percent(row.ytd, ytd.income),
        })
        .collect();
    percent_rows.push(PercentRow {
        label: "Net Pay".to_string(),
        cells: column_totals
            .iter()
            .map(|t| percent(t.net, t.income))
            .collect(),
        ytd: percent(ytd.net, ytd.income),
    });
    Sheet {
        year,
        columns: checks
            .iter()
            .map(|p| Column {
                id: p.id,
                date: p.date,
            })
            .collect(),
        rows,
        net: column_totals.iter().map(|t| t.net).collect(),
        net_ytd: ytd.net,
        percent_rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn field(id: FieldId, name: &str, kind: Kind, position: i64, archived: bool) -> Field {
        Field {
            id,
            name: name.to_string(),
            kind,
            position,
            archived,
        }
    }

    fn check(id: PaycheckId, (y, m, d): (i32, u32, u32), amounts: &[(FieldId, i64)]) -> Paycheck {
        Paycheck {
            id,
            date: NaiveDate::from_ymd_opt(y, m, d).unwrap(),
            amounts: amounts
                .iter()
                .map(|&(f, c)| (f, Cents(c)))
                .collect::<BTreeMap<_, _>>(),
        }
    }

    /// Salary (income), Tax, Retirement (deductions); Bonus (income) sorts
    /// after Tax by position but before it by kind.
    fn fields() -> Vec<Field> {
        vec![
            field(1, "Salary", Kind::Income, 0, false),
            field(2, "Tax", Kind::Deduction, 1, false),
            field(3, "Bonus", Kind::Income, 2, false),
            field(4, "Retirement", Kind::Deduction, 3, false),
        ]
    }

    #[test]
    fn net_is_income_minus_deductions() {
        let t = totals([
            (Kind::Income, Cents(400_000)),
            (Kind::Deduction, Cents(60_000)),
            (Kind::Income, Cents(10_000)),
            (Kind::Deduction, Cents(-500)),
        ]);
        assert_eq!(
            t,
            Totals {
                income: Cents(410_000),
                net: Cents(350_500)
            }
        );
    }

    #[test]
    fn percent_shows_two_decimals() {
        assert_eq!(show(percent(Cents(60_000), Cents(400_000))), "15.00%");
        assert_eq!(show(percent(Cents(109_520), Cents(400_000))), "27.38%");
    }

    #[test]
    fn percent_rounds_half_away_from_zero() {
        assert_eq!(percent(Cents(1), Cents(3)), Some(Percent(3333)));
        assert_eq!(percent(Cents(2), Cents(3)), Some(Percent(6667)));
        assert_eq!(percent(Cents(-2), Cents(3)), Some(Percent(-6667)));
        assert_eq!(show(percent(Cents(-2), Cents(3))), "-66.67%");
    }

    #[test]
    fn percent_is_a_dash_when_income_is_zero() {
        assert_eq!(percent(Cents(100), Cents(0)), None);
        assert_eq!(show(None), "—");
    }

    #[test]
    fn rows_put_income_before_deductions_each_in_position_order() {
        let fields = fields();
        let names: Vec<_> = visible_fields(&fields, &[])
            .iter()
            .map(|f| f.name.as_str())
            .collect();
        assert_eq!(names, ["Salary", "Bonus", "Tax", "Retirement"]);
    }

    #[test]
    fn an_archived_field_appears_only_in_years_a_paycheck_used_it() {
        let mut fields = fields();
        fields[3].archived = true;
        let checks = [
            check(1, (2025, 12, 19), &[(1, 100), (4, 10)]),
            check(2, (2026, 1, 2), &[(1, 100)]),
        ];
        let names = |year: i32| -> Vec<String> {
            sheet(year, &fields, &checks)
                .rows
                .into_iter()
                .map(|r| r.name)
                .collect()
        };
        assert!(names(2025).contains(&"Retirement".to_string()));
        assert!(!names(2026).contains(&"Retirement".to_string()));
    }

    #[test]
    fn the_sheet_holds_only_the_years_paychecks_oldest_first() {
        let checks = [
            check(1, (2026, 1, 16), &[(1, 100)]),
            check(2, (2025, 12, 19), &[(1, 100)]),
            check(3, (2026, 1, 2), &[(1, 100)]),
        ];
        let s = sheet(2026, &fields(), &checks);
        let ids: Vec<_> = s.columns.iter().map(|c| c.id).collect();
        assert_eq!(ids, [3, 1]);
    }

    #[test]
    fn a_field_a_paycheck_lacks_is_a_blank_cell_and_zero_in_the_percent_block() {
        let checks = [check(1, (2026, 1, 2), &[(1, 400_000)])];
        let s = sheet(2026, &fields(), &checks);
        let tax = s.rows.iter().find(|r| r.name == "Tax").unwrap();
        assert_eq!(tax.cells, [None]);
        assert_eq!(tax.ytd, Cents::ZERO);
        let tax_pct = s.percent_rows.iter().find(|r| r.label == "Tax").unwrap();
        assert_eq!(tax_pct.cells, [Some(Percent(0))]);
    }

    #[test]
    fn the_sheet_totals_each_column_and_the_year() {
        let checks = [
            check(1, (2026, 1, 2), &[(1, 400_000), (2, 60_000), (4, 20_000)]),
            check(
                2,
                (2026, 1, 16),
                &[(1, 400_000), (3, 50_000), (2, 70_000), (4, 20_000)],
            ),
        ];
        let s = sheet(2026, &fields(), &checks);
        assert_eq!(s.net, [Cents(320_000), Cents(360_000)]);
        assert_eq!(s.net_ytd, Cents(680_000));
        let salary = &s.rows[0];
        assert_eq!(salary.cells, [Some(Cents(400_000)), Some(Cents(400_000))]);
        assert_eq!(salary.ytd, Cents(800_000));
    }

    #[test]
    fn the_percent_block_lists_deductions_then_net_pay() {
        let checks = [check(
            1,
            (2026, 1, 2),
            &[(1, 400_000), (2, 60_000), (4, 20_000)],
        )];
        let s = sheet(2026, &fields(), &checks);
        let labels: Vec<_> = s.percent_rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Tax", "Retirement", "Net Pay"]);
        assert_eq!(s.percent_rows[2].cells, [Some(Percent(8000))]);
    }

    #[test]
    fn the_ytd_percent_is_the_ratio_of_the_sums_not_an_average() {
        // 10% and 30% average to 20%, but 1,000 of 4,000 is 25%.
        let checks = [
            check(1, (2026, 1, 2), &[(1, 100_000), (2, 10_000)]),
            check(2, (2026, 1, 16), &[(1, 300_000), (2, 90_000)]),
        ];
        let s = sheet(2026, &fields(), &checks);
        assert_eq!(s.percent_rows[0].ytd, Some(Percent(2500)));
    }

    #[test]
    fn a_year_with_no_paychecks_has_rows_but_no_columns_and_dashes_for_percentages() {
        let s = sheet(2027, &fields(), &[]);
        assert!(s.columns.is_empty());
        assert_eq!(s.rows.len(), 4);
        assert_eq!(s.net_ytd, Cents::ZERO);
        assert_eq!(s.percent_rows.last().unwrap().ytd, None);
    }

    #[test]
    fn changing_a_fields_kind_changes_past_net() {
        let mut fields = fields();
        let checks = [check(1, (2026, 1, 2), &[(1, 400_000), (3, 50_000)])];
        assert_eq!(sheet(2026, &fields, &checks).net, [Cents(450_000)]);
        fields[2].kind = Kind::Deduction;
        assert_eq!(sheet(2026, &fields, &checks).net, [Cents(350_000)]);
    }
}
