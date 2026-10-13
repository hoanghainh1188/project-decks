//! MSPDI calendar exceptions written as a `WeekDay` with `DayType` 0. The fixture is
//! handwritten to the public schema; it is not exported from Microsoft Project.

use sheafly_engine::calendar::TimeRange;
use sheafly_engine::lang::Lang;
use sheafly_engine::mspdi::{read_mspdi, read_mspdi_with_report, write_mspdi};
use sheafly_engine::mspdi_report::ImportNote;
use sheafly_engine::timeutil::timestamp;

const FIXTURE: &str = include_str!("fixtures/daytype0.xml");

fn at(day: u32, hour: u32, minute: u32) -> i64 {
    // Monday 2026-01-05 is day 0.
    timestamp(2026, 1, 5 + day, hour, minute)
}

#[test]
fn a_day_type_0_day_off_and_a_span_with_hours_are_read() {
    let project = read_mspdi(FIXTURE).unwrap();
    let calendar = project.calendar();
    assert_eq!(calendar.exceptions.get("2026-01-13"), Some(&Vec::new()), "day off");
    for day in ["2026-01-19", "2026-01-20", "2026-01-21"] {
        assert_eq!(
            calendar.exceptions.get(day),
            Some(&vec![TimeRange::new(9 * 60, 13 * 60)]),
            "{day}"
        );
    }
    assert!(!calendar.exceptions.contains_key("2026-01-22"));
    // The seven weekdays are untouched.
    assert_eq!(calendar.week[0].len(), 2);
    assert!(calendar.week[5].is_empty() && calendar.week[6].is_empty());
}

#[test]
fn both_kinds_in_one_file_are_kept() {
    let project = read_mspdi(FIXTURE).unwrap();
    let calendar = project.calendar();
    assert_eq!(calendar.exceptions.get("2026-01-26"), Some(&Vec::new()));
    assert_eq!(calendar.exceptions.get("2026-01-27"), Some(&Vec::new()));
    assert_eq!(calendar.exceptions.get("2026-01-13"), Some(&Vec::new()));
}

#[test]
fn the_schedule_skips_a_day_type_0_day_off() {
    let project = read_mspdi(FIXTURE).unwrap();
    let task = project.task(1).unwrap();
    // Monday and Wednesday: Tuesday 13 January is off.
    assert_eq!(task.start, at(7, 8, 0));
    assert_eq!(task.finish, at(9, 17, 0));
}

#[test]
fn a_based_calendar_keeps_its_exception_as_its_own() {
    let project = read_mspdi(FIXTURE).unwrap();
    let based = project.calendars.iter().find(|c| c.id == 2).unwrap();
    let own = &based.base.as_ref().expect("based on calendar 1").exceptions;
    assert_eq!(own.keys().collect::<Vec<_>>(), vec!["2026-02-02"]);
    // The merged calendar also has the day off the base gives.
    assert_eq!(based.exceptions.get("2026-02-02"), Some(&Vec::new()));
    assert_eq!(based.exceptions.get("2026-01-13"), Some(&Vec::new()));
}

#[test]
fn an_entry_without_dates_is_left_out_and_noted_in_both_languages() {
    let (_, report) = read_mspdi_with_report(FIXTURE).unwrap();
    for name in ["Standard", "Based"] {
        assert!(
            report
                .notes
                .contains(&ImportNote::ExceptionWithoutDates { calendar: name.into() }),
            "{name}: {:?}",
            report.notes
        );
    }
    assert!(report
        .message(Lang::Vi)
        .unwrap()
        .contains("Lịch “Standard”: một ngày ngoại lệ không có ngày bắt đầu"));
    assert!(report
        .message(Lang::En)
        .unwrap()
        .contains("Calendar “Based”: an exception with no start date was left out."));
}

#[test]
fn what_was_read_survives_a_write_and_read_back() {
    let project = read_mspdi(FIXTURE).unwrap();
    let back = read_mspdi(&write_mspdi(&project).unwrap()).unwrap();
    assert_eq!(back.calendar().exceptions, project.calendar().exceptions);
    let (before, after) = (project.task(1).unwrap(), back.task(1).unwrap());
    assert_eq!((after.start, after.finish), (before.start, before.finish));
}

#[test]
fn a_day_type_0_inside_a_work_week_does_not_become_an_exception() {
    let original = include_str!("fixtures/workweeks.xml");
    let xml = original.replace(
        "<WeekDay><DayType>2</DayType><DayWorking>0</DayWorking></WeekDay>",
        "<WeekDay><DayType>0</DayType><DayWorking>0</DayWorking>\
         <TimePeriod><FromDate>2026-01-22T00:00:00</FromDate></TimePeriod></WeekDay>",
    );
    assert_ne!(xml, original, "the fixture still has the day to replace");
    let (project, report) = read_mspdi_with_report(&xml).unwrap();
    assert!(project.calendar().exceptions.is_empty());
    assert!(!report
        .notes
        .iter()
        .any(|n| matches!(n, ImportNote::ExceptionWithoutDates { .. })));
}

#[test]
fn a_weekday_without_a_usable_day_type_is_dropped_without_a_note() {
    for day in [
        "<WeekDay><DayWorking>0</DayWorking></WeekDay>",
        "<WeekDay><DayType>Monday</DayType><DayWorking>0</DayWorking></WeekDay>",
    ] {
        let xml = FIXTURE.replace("<WeekDay><DayType>1</DayType>", &format!("{day}<WeekDay><DayType>1</DayType>"));
        assert_ne!(xml, FIXTURE);
        let (_, report) = read_mspdi_with_report(&xml).unwrap();
        let noted = report
            .notes
            .iter()
            .filter(|n| matches!(n, ImportNote::ExceptionWithoutDates { calendar } if calendar == "Standard"))
            .count();
        assert_eq!(noted, 1, "only the real DayType 0 entry is noted: {day}");
    }
}

#[test]
fn a_working_day_type_0_without_hours_is_ignored() {
    let xml = FIXTURE.replace(
        "<WeekDay><DayType>1</DayType>",
        "<WeekDay><DayType>0</DayType><DayWorking>1</DayWorking>\
         <TimePeriod><FromDate>2026-03-02T00:00:00</FromDate></TimePeriod></WeekDay>\
         <WeekDay><DayType>1</DayType>",
    );
    let project = read_mspdi(&xml).unwrap();
    assert!(!project.calendar().exceptions.contains_key("2026-03-02"));
}
