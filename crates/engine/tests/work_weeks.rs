//! Work weeks of a calendar: commands, scheduling, files and MSPDI.

use chrono::Datelike;

use sheafly_engine::calendar::TimeRange;
use sheafly_engine::command::Command;
use sheafly_engine::error::{EngineError, Invalid};
use sheafly_engine::mspdi::{read_mspdi, write_mspdi};
use sheafly_engine::store::{load, save};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::Document;

const DAY: i64 = 480;

fn at(day: i64, hour: u32, minute: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(day))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, minute)
}

/// Mon and Tue 09:00–13:00, every other day off.
fn short_week() -> Vec<Vec<TimeRange>> {
    let half = vec![TimeRange::new(9 * 60, 13 * 60)];
    vec![half.clone(), half, vec![], vec![], vec![], vec![], vec![]]
}

fn set_short_weeks(from: &str, to: &str) -> Command {
    Command::SetCalendarWorkWeek {
        calendar_id: 1,
        name: "Tuần ngắn".into(),
        from: from.into(),
        to: to.into(),
        week: short_week(),
    }
}

/// One 3-day task from Monday 2026-01-12.
fn three_day_task() -> Document {
    let mut doc = Document::new_default();
    doc.apply(Command::SetProjectStart { start: at(7, 8, 0) }).unwrap();
    doc.apply(Command::SetDuration { uid: 1, minutes: 3 * DAY }).unwrap();
    doc
}

#[test]
fn a_work_week_changes_the_schedule_and_undo_brings_it_back() {
    let mut doc = three_day_task();
    assert_eq!(doc.project.task(1).unwrap().finish, at(9, 17, 0), "default week");
    doc.apply(set_short_weeks("2026-01-13", "2026-01-25")).unwrap();
    // 20 h: a default Monday 12 (8 h), then four-hour days on Tue 13, Mon 19 and Tue 20,
    // and the last 4 h on the default Monday 26 (the work week ended on the 25th).
    let task = doc.project.task(1).unwrap();
    assert_eq!(task.start, at(7, 8, 0));
    assert_eq!(task.finish, at(21, 12, 0));
    doc.undo().unwrap();
    assert_eq!(doc.project.task(1).unwrap().finish, at(9, 17, 0));
    doc.redo().unwrap();
    assert_eq!(doc.project.task(1).unwrap().finish, at(21, 12, 0));
    doc.apply(Command::ClearCalendarWorkWeek { calendar_id: 1, from: "2026-01-13".into() })
        .unwrap();
    assert_eq!(doc.project.task(1).unwrap().finish, at(9, 17, 0));
}

#[test]
fn bad_work_weeks_are_refused_with_a_reason() {
    let mut doc = three_day_task();
    doc.apply(set_short_weeks("2026-01-13", "2026-01-25")).unwrap();
    let overlap = doc.apply(set_short_weeks("2026-01-20", "2026-02-01")).unwrap_err();
    assert_eq!(overlap, EngineError::Invalid(Invalid::WorkWeekOverlaps));
    let backwards = doc.apply(set_short_weeks("2026-02-01", "2026-01-01")).unwrap_err();
    assert_eq!(backwards, EngineError::Invalid(Invalid::BadDate));
    let missing = doc
        .apply(Command::ClearCalendarWorkWeek { calendar_id: 1, from: "2026-03-01".into() })
        .unwrap_err();
    assert_eq!(missing, EngineError::Invalid(Invalid::NoSuchWorkWeek));
    let six_days = doc
        .apply(Command::SetCalendarWorkWeek {
            calendar_id: 1,
            name: String::new(),
            from: "2026-03-01".into(),
            to: "2026-03-07".into(),
            week: short_week()[..6].to_vec(),
        })
        .unwrap_err();
    assert_eq!(six_days, EngineError::Invalid(Invalid::WeekNeedsSevenDays));
    assert_eq!(doc.project.calendar().work_weeks.len(), 1, "failures change nothing");
}

#[test]
fn work_weeks_survive_save_and_open() {
    let dir = std::env::temp_dir().join(format!("sheafly-ww-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("weeks.sheafly").to_string_lossy().into_owned();
    let mut doc = three_day_task();
    doc.apply(set_short_weeks("2026-01-13", "2026-01-25")).unwrap();
    save(&doc, &path).unwrap();
    let back = load(&path).unwrap();
    assert_eq!(back.project.calendar().work_weeks, doc.project.calendar().work_weeks);
    assert_eq!(back.project.task(1).unwrap().finish, at(21, 12, 0));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn work_weeks_round_trip_through_mspdi() {
    let mut doc = three_day_task();
    doc.apply(set_short_weeks("2026-01-13", "2026-01-25")).unwrap();
    let xml = write_mspdi(&doc.project).unwrap();
    assert!(xml.contains("<WorkWeeks>") && xml.contains("<Name>Tuần ngắn</Name>"));
    let back = read_mspdi(&xml).unwrap();
    assert_eq!(back.calendar().work_weeks, doc.project.calendar().work_weeks);
    assert_eq!(back.calendar().week, doc.project.calendar().week, "the default week is untouched");
    let (before, after) = (doc.project.task(1).unwrap(), back.task(1).unwrap());
    assert_eq!((after.start, after.finish), (before.start, before.finish));
}

#[test]
fn a_handwritten_file_with_work_weeks_is_read() {
    let project = read_mspdi(include_str!("fixtures/workweeks.xml")).unwrap();
    let calendar = project.calendar();
    assert_eq!(calendar.work_weeks.len(), 1, "the overlapping week is dropped");
    let week = &calendar.work_weeks[0];
    assert_eq!(week.name, "Short weeks");
    assert_eq!(week.from.to_string(), "2026-01-12");
    assert_eq!(week.to.to_string(), "2026-01-25");
    assert_eq!(week.week[0], vec![TimeRange::new(9 * 60, 13 * 60)]);
    assert!(week.week[2].is_empty());
    // The default week kept its own hours.
    assert_eq!(calendar.week[0].len(), 2);
    let task = project.task(1).unwrap();
    assert_eq!(task.finish, at(21, 17, 0));
}

#[test]
fn a_partial_work_week_keeps_the_default_hours_of_unlisted_days_and_dates_may_omit_the_time() {
    let xml = include_str!("fixtures/workweeks.xml")
        .replace("2026-01-12T00:00:00", "2026-01-12")
        .replace("2026-01-25T23:59:00", "2026-01-25");
    // Only Monday is listed: it works 09:00–13:00; the other days stay as the default week.
    let start = xml.find("<WeekDays>\n            <WeekDay><DayType>2").unwrap();
    let end = xml[start..].find("</WeekDays>").unwrap() + start;
    let partial = format!(
        "{}<WeekDays><WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>09:00:00</FromTime><ToTime>13:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>{}",
        &xml[..start],
        &xml[end..]
    );
    let project = read_mspdi(&partial).unwrap();
    let week = &project.calendar().work_weeks[0].week;
    assert_eq!(week[0], vec![TimeRange::new(9 * 60, 13 * 60)]);
    assert_eq!(week[1].len(), 2, "Tuesday is unlisted, so it keeps the default hours");
    assert!(week[5].is_empty());
}
