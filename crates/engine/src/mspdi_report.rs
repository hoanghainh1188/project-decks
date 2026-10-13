//! What importing an MSPDI file changed or left out, so the app can say so.
//!
//! The engine reads a file its own way: actual dates and remaining amounts win over the
//! percent complete, work weeks may not overlap, and every task is scheduled again. Each
//! of those can make the project differ from what Project showed for the same file.

use crate::lang::Lang;
use crate::recovery::span;
use crate::timeutil::civil;

/// How many items of one kind the message names before summing up the rest.
const LISTED: usize = 10;

#[derive(Clone, Debug, PartialEq)]
pub enum ImportNote {
    /// A work week that overlaps an earlier one in the same calendar was left out.
    WorkWeekOverlaps { calendar: String, name: String },
    /// A work week with no usable `FromDate` was left out.
    WorkWeekWithoutDates { calendar: String },
    /// A calendar exception (a `WeekDay` with `DayType` 0) with no usable `FromDate` was
    /// left out.
    ExceptionWithoutDates { calendar: String },
    /// The percent complete the file gave was replaced by the one its actuals give.
    PercentFromActuals { task: String, file: f64, used: f64 },
    /// Values of a custom field of a kind Sheafly does not keep (cost, date, duration,
    /// flag) were left out.
    FieldSkipped { name: String, values: usize },
    /// Values of a number field that were not numbers were left out.
    FieldValuesUnread { name: String, values: usize },
    /// A task the engine schedules on other dates than the file gave.
    Moved {
        task: String,
        before: (i64, i64),
        after: (i64, i64),
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImportReport {
    pub notes: Vec<ImportNote>,
}

impl ImportReport {
    /// The warning to show after importing, in `lang`, if there is anything to say.
    pub fn message(&self, lang: Lang) -> Option<String> {
        let weeks: Vec<String> = self
            .notes
            .iter()
            .filter_map(|note| week_line(note, lang))
            .collect();
        let percents: Vec<String> = self
            .notes
            .iter()
            .filter_map(|note| match note {
                ImportNote::PercentFromActuals { task, file, used } => Some(format!(
                    "• {task}: {} → {}",
                    percent(*file, lang),
                    percent(*used, lang)
                )),
                _ => None,
            })
            .collect();
        let moved: Vec<String> = self
            .notes
            .iter()
            .filter_map(|note| match note {
                ImportNote::Moved {
                    task,
                    before,
                    after,
                } => Some(moved_line(task, *before, *after, lang)),
                _ => None,
            })
            .collect();
        let skipped: Vec<String> = self
            .notes
            .iter()
            .filter_map(|note| match note {
                ImportNote::FieldSkipped { name, values } => {
                    Some(field_line(name, *values, false, lang))
                }
                ImportNote::FieldValuesUnread { name, values } => {
                    Some(field_line(name, *values, true, lang))
                }
                _ => None,
            })
            .collect();
        let parts: Vec<String> = [
            section(&weeks, heading_weeks(lang), lang),
            section(&skipped, heading_fields(lang), lang),
            section(&percents, heading_percents(percents.len(), lang), lang),
            section(&moved, heading_moved(moved.len(), lang), lang),
        ]
        .into_iter()
        .flatten()
        .collect();
        (!parts.is_empty()).then(|| parts.join("\n\n"))
    }
}

fn week_line(note: &ImportNote, lang: Lang) -> Option<String> {
    Some(match (note, lang) {
        (ImportNote::WorkWeekOverlaps { calendar, name }, Lang::Vi) => {
            format!("• Lịch “{calendar}”: tuần “{name}” trùng ngày với tuần trước, đã bỏ.")
        }
        (ImportNote::WorkWeekOverlaps { calendar, name }, Lang::En) => {
            format!(
                "• Calendar “{calendar}”: week “{name}” overlaps an earlier one and was left out."
            )
        }
        (ImportNote::WorkWeekWithoutDates { calendar }, Lang::Vi) => {
            format!("• Lịch “{calendar}”: một tuần làm việc không có ngày bắt đầu, đã bỏ.")
        }
        (ImportNote::WorkWeekWithoutDates { calendar }, Lang::En) => {
            format!("• Calendar “{calendar}”: a work week with no start date was left out.")
        }
        (ImportNote::ExceptionWithoutDates { calendar }, Lang::Vi) => {
            format!("• Lịch “{calendar}”: một ngày ngoại lệ không có ngày bắt đầu, đã bỏ.")
        }
        (ImportNote::ExceptionWithoutDates { calendar }, Lang::En) => {
            format!("• Calendar “{calendar}”: an exception with no start date was left out.")
        }
        _ => return None,
    })
}

/// One field's left-out values: of a kind Sheafly does not keep, or (`not_numbers`) text
/// in a number field.
fn field_line(name: &str, values: usize, not_numbers: bool, lang: Lang) -> String {
    let one = values == 1;
    match (lang, not_numbers) {
        (Lang::Vi, false) => {
            format!("• “{name}”: {values} giá trị (Sheafly chỉ giữ trường chữ, số và danh sách)")
        }
        (Lang::Vi, true) => format!("• “{name}”: {values} giá trị không phải số"),
        (Lang::En, false) => format!(
            "• “{name}”: {values} {} (Sheafly keeps only text, number, and list fields)",
            if one { "value" } else { "values" }
        ),
        (Lang::En, true) => format!(
            "• “{name}”: {values} {}",
            if one { "value that is not a number" } else { "values that are not numbers" }
        ),
    }
}

fn heading_fields(lang: Lang) -> String {
    match lang {
        Lang::Vi => "Một số giá trị trường tùy chỉnh trong tệp đã bỏ:".into(),
        Lang::En => "Some custom field values in the file were left out:".into(),
    }
}

fn heading_weeks(lang: Lang) -> String {
    match lang {
        Lang::Vi => "Một số tuần làm việc hoặc ngày ngoại lệ trong tệp không đọc được:".into(),
        Lang::En => "Some work weeks or exceptions in the file could not be read:".into(),
    }
}

fn heading_percents(count: usize, lang: Lang) -> String {
    match lang {
        Lang::Vi => format!(
            "{count} công việc có % hoàn thành khác với ngày và phần còn lại trong tệp; \
             đã dùng số theo ngày và phần còn lại:"
        ),
        Lang::En => format!(
            "{count} {} a % complete that disagrees with the actual dates and remaining \
             amounts in the file; those were used instead:",
            if count == 1 { "task has" } else { "tasks have" }
        ),
    }
}

fn heading_moved(count: usize, lang: Lang) -> String {
    match lang {
        Lang::Vi => format!(
            "Sheafly tính lại lịch: {count} công việc có ngày khác với tệp. Hãy kiểm tra \
             trước khi lưu."
        ),
        Lang::En => format!(
            "Sheafly scheduled the project again: {count} {} other dates than in the file. \
             Check them before you save.",
            if count == 1 { "task has" } else { "tasks have" }
        ),
    }
}

/// The days, or the times too when the days alone look the same.
fn moved_line(task: &str, before: (i64, i64), after: (i64, i64), lang: Lang) -> String {
    let (old, new) = (span(before, lang), span(after, lang));
    if old != new {
        return format!("• {task}: {old} → {new}");
    }
    let at = |ms: i64| {
        let minute = civil(ms).1;
        format!("{:02}:{:02}", minute / 60, minute % 60)
    };
    let clock = |(start, finish): (i64, i64)| format!("{}–{}", at(start), at(finish));
    format!("• {task}: {old}, {} → {}", clock(before), clock(after))
}

fn section(lines: &[String], heading: String, lang: Lang) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let mut out = vec![heading];
    out.extend(lines.iter().take(LISTED).cloned());
    if lines.len() > LISTED {
        let rest = lines.len() - LISTED;
        out.push(match lang {
            Lang::Vi => format!("… và {rest} mục khác."),
            Lang::En => format!("… and {rest} more."),
        });
    }
    Some(out.join("\n"))
}

fn percent(value: f64, lang: Lang) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    let text = if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded:.1}")
    };
    match lang {
        Lang::Vi => format!("{} %", text.replace('.', ",")),
        Lang::En => format!("{text}%"),
    }
}
