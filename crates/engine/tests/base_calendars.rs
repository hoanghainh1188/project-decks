//! Calendars based on another: a holiday on the project calendar reaches the resource
//! calendars based on it, while their own hours stay theirs.

use chrono::NaiveDate;

use sheafly_engine::calendar::TimeRange;
use sheafly_engine::command::Command;
use sheafly_engine::error::{EngineError, Invalid};
use sheafly_engine::mspdi::{read_mspdi, write_mspdi};
use sheafly_engine::store::{load, save};
use sheafly_engine::Document;

fn day(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").unwrap()
}

fn half_day() -> Vec<TimeRange> {
    vec![TimeRange::new(8 * 60, 12 * 60)]
}

/// Project calendar 1, and calendar 2 "An" based on it with half-day Fridays.
fn with_based_calendar() -> Document {
    let mut doc = Document::new_default();
    doc.apply(Command::CreateCalendar {
        name: "An".into(),
        base_id: Some(1),
    })
    .unwrap();
    let mut week = doc.project.calendar_by_id(2).unwrap().week.to_vec();
    week[4] = half_day();
    doc.apply(Command::SetCalendarWeek {
        calendar_id: 2,
        week,
    })
    .unwrap();
    doc
}

#[test]
fn a_based_calendar_owns_only_what_it_sets() {
    let doc = with_based_calendar();
    let an = doc.project.calendar_by_id(2).unwrap();
    let own = an.base.as_ref().unwrap();
    assert_eq!(own.id, 1);
    assert_eq!(own.week[4], Some(half_day()));
    assert!(own
        .week
        .iter()
        .enumerate()
        .all(|(index, day)| index == 4 || day.is_none()));
    assert!(own.exceptions.is_empty());
}

#[test]
fn a_holiday_on_the_base_reaches_the_based_calendar_and_its_own_days_stay() {
    let mut doc = with_based_calendar();
    doc.apply(Command::SetCalendarExceptions {
        calendar_id: 1,
        from: "2026-01-06".into(),
        to: "2026-01-06".into(),
        ranges: vec![],
    })
    .unwrap();
    let an = doc.project.calendar_by_id(2).unwrap();
    assert!(
        an.ranges_on(day("2026-01-06")).is_empty(),
        "the project holiday"
    );
    assert_eq!(
        an.ranges_on(day("2026-01-09")),
        half_day().as_slice(),
        "An's own Friday"
    );
    // Changing the base week reaches the days An does not set.
    let mut week = doc.project.calendar().week.to_vec();
    week[0] = half_day();
    doc.apply(Command::SetCalendarWeek {
        calendar_id: 1,
        week,
    })
    .unwrap();
    let an = doc.project.calendar_by_id(2).unwrap();
    assert_eq!(an.ranges_on(day("2026-01-12")), half_day().as_slice());
    assert_eq!(an.ranges_on(day("2026-01-09")), half_day().as_slice());
    // Undo goes back through both.
    doc.undo().unwrap();
    doc.undo().unwrap();
    let an = doc.project.calendar_by_id(2).unwrap();
    assert_eq!(an.ranges_on(day("2026-01-06")).len(), 2);
}

#[test]
fn an_exception_on_the_based_calendar_overrides_the_base() {
    let mut doc = with_based_calendar();
    doc.apply(Command::SetCalendarExceptions {
        calendar_id: 1,
        from: "2026-01-06".into(),
        to: "2026-01-06".into(),
        ranges: vec![],
    })
    .unwrap();
    doc.apply(Command::SetCalendarExceptions {
        calendar_id: 2,
        from: "2026-01-06".into(),
        to: "2026-01-06".into(),
        ranges: half_day(),
    })
    .unwrap();
    let an = doc.project.calendar_by_id(2).unwrap();
    assert_eq!(an.ranges_on(day("2026-01-06")), half_day().as_slice());
    assert!(doc
        .project
        .calendar()
        .ranges_on(day("2026-01-06"))
        .is_empty());
}

#[test]
fn bases_never_loop_and_standing_alone_keeps_the_hours() {
    let mut doc = with_based_calendar();
    let looped = doc
        .apply(Command::SetCalendarBase {
            calendar_id: 1,
            base_id: Some(2),
        })
        .unwrap_err();
    assert_eq!(looped, EngineError::Invalid(Invalid::CalendarBaseLoop));
    let itself = doc
        .apply(Command::SetCalendarBase {
            calendar_id: 2,
            base_id: Some(2),
        })
        .unwrap_err();
    assert_eq!(itself, EngineError::Invalid(Invalid::CalendarBaseLoop));
    doc.apply(Command::SetCalendarBase {
        calendar_id: 2,
        base_id: None,
    })
    .unwrap();
    let an = doc.project.calendar_by_id(2).unwrap().clone();
    assert!(an.base.is_none());
    assert_eq!(an.week[4], half_day());
    // A later holiday on calendar 1 no longer reaches it.
    doc.apply(Command::SetCalendarExceptions {
        calendar_id: 1,
        from: "2026-01-06".into(),
        to: "2026-01-06".into(),
        ranges: vec![],
    })
    .unwrap();
    assert_eq!(
        doc.project
            .calendar_by_id(2)
            .unwrap()
            .ranges_on(day("2026-01-06"))
            .len(),
        2
    );
}

#[test]
fn an_existing_calendar_can_be_based_on_another_and_keeps_its_differences() {
    let mut doc = Document::new_default();
    doc.apply(Command::CreateCalendar {
        name: "Máy".into(),
        base_id: None,
    })
    .unwrap();
    let mut week = doc.project.calendar_by_id(2).unwrap().week.to_vec();
    week[5] = half_day();
    doc.apply(Command::SetCalendarWeek {
        calendar_id: 2,
        week,
    })
    .unwrap();
    doc.apply(Command::SetCalendarBase {
        calendar_id: 2,
        base_id: Some(1),
    })
    .unwrap();
    let own = doc.project.calendar_by_id(2).unwrap().base.clone().unwrap();
    assert_eq!(own.week[5], Some(half_day()), "the Saturday it works");
    assert_eq!(
        own.week[0], None,
        "a Monday like the base's follows the base"
    );
}

#[test]
fn based_calendars_survive_save_open_and_mspdi() {
    let mut doc = with_based_calendar();
    doc.apply(Command::SetCalendarExceptions {
        calendar_id: 1,
        from: "2026-01-06".into(),
        to: "2026-01-06".into(),
        ranges: vec![],
    })
    .unwrap();
    let dir = std::env::temp_dir().join(format!("sheafly-base-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("base.sheafly").to_string_lossy().into_owned();
    save(&doc, &path).unwrap();
    let opened = load(&path).unwrap();
    assert_eq!(opened.project.calendars, doc.project.calendars);
    std::fs::remove_dir_all(dir).ok();

    let xml = write_mspdi(&doc.project).unwrap();
    assert!(xml.contains("<BaseCalendarUID>1</BaseCalendarUID>"));
    let back = read_mspdi(&xml).unwrap();
    assert_eq!(back.calendars, doc.project.calendars);
}

#[test]
fn a_project_style_resource_calendar_lists_only_its_own_days() {
    // Handwritten to the public MSPDI schema, as Project writes a resource calendar:
    // only the days and exceptions it changes, and the base's UID.
    let xml = r#"<Project xmlns="http://schemas.microsoft.com/project">
      <StartDate>2026-01-05T08:00:00</StartDate><CalendarUID>1</CalendarUID>
      <Calendars>
        <Calendar><UID>1</UID><Name>Standard</Name><IsBaseCalendar>1</IsBaseCalendar>
          <BaseCalendarUID>-1</BaseCalendarUID>
          <WeekDays>
            <WeekDay><DayType>1</DayType><DayWorking>0</DayWorking></WeekDay>
            <WeekDay><DayType>2</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>17:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
            <WeekDay><DayType>3</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>17:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
            <WeekDay><DayType>4</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>17:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
            <WeekDay><DayType>5</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>17:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
            <WeekDay><DayType>6</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>17:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
            <WeekDay><DayType>7</DayType><DayWorking>0</DayWorking></WeekDay>
          </WeekDays>
          <Exceptions><Exception><TimePeriod><FromDate>2026-01-06T00:00:00</FromDate><ToDate>2026-01-06T23:59:00</ToDate></TimePeriod><DayWorking>0</DayWorking></Exception></Exceptions>
        </Calendar>
        <Calendar><UID>2</UID><Name>An</Name><IsBaseCalendar>0</IsBaseCalendar>
          <BaseCalendarUID>1</BaseCalendarUID>
          <WeekDays>
            <WeekDay><DayType>6</DayType><DayWorking>1</DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00</FromTime><ToTime>12:00:00</ToTime></WorkingTime></WorkingTimes></WeekDay>
          </WeekDays>
        </Calendar>
      </Calendars>
    </Project>"#;
    let project = read_mspdi(xml).unwrap();
    let an = project.calendar_by_id(2).unwrap();
    assert_eq!(an.base.as_ref().unwrap().id, 1);
    assert!(
        project.calendar_by_id(1).unwrap().base.is_none(),
        "-1 is no base"
    );
    assert_eq!(
        an.ranges_on(day("2026-01-09")),
        half_day().as_slice(),
        "its own Friday"
    );
    assert_eq!(
        an.ranges_on(day("2026-01-05")),
        &[TimeRange::new(8 * 60, 17 * 60)],
        "Monday from the base"
    );
    assert!(
        an.ranges_on(day("2026-01-06")).is_empty(),
        "the base's holiday"
    );
}

#[test]
fn holidays_on_a_based_project_calendar_stick_and_inherited_ones_cannot_be_removed() {
    let mut doc = Document::new_default();
    // The project calendar (1) is based on a company calendar (2).
    doc.apply(Command::CreateCalendar {
        name: "Công ty".into(),
        base_id: None,
    })
    .unwrap();
    doc.apply(Command::SetCalendarBase {
        calendar_id: 1,
        base_id: Some(2),
    })
    .unwrap();
    doc.apply(Command::AddHoliday {
        date: "2026-01-07".into(),
    })
    .unwrap();
    assert!(
        doc.project
            .calendar()
            .ranges_on(day("2026-01-07"))
            .is_empty(),
        "kept after refresh"
    );
    doc.apply(Command::SetCalendarExceptions {
        calendar_id: 2,
        from: "2026-01-06".into(),
        to: "2026-01-06".into(),
        ranges: vec![],
    })
    .unwrap();
    assert!(
        doc.project
            .calendar()
            .ranges_on(day("2026-01-06"))
            .is_empty(),
        "the company holiday"
    );
    let inherited = doc
        .apply(Command::RemoveHoliday {
            date: "2026-01-06".into(),
        })
        .unwrap_err();
    assert_eq!(inherited, EngineError::Invalid(Invalid::InheritedFromBase));
    let cleared = doc
        .apply(Command::ClearCalendarExceptions {
            calendar_id: 1,
            from: "2026-01-05".into(),
            to: "2026-01-07".into(),
        })
        .unwrap_err();
    assert_eq!(cleared, EngineError::Invalid(Invalid::InheritedFromBase));
    doc.apply(Command::RemoveHoliday {
        date: "2026-01-07".into(),
    })
    .unwrap();
    assert_eq!(doc.project.calendar().ranges_on(day("2026-01-07")).len(), 2);
}

#[test]
fn a_based_calendar_can_override_part_of_an_inherited_work_week() {
    let mut doc = with_based_calendar();
    let half = vec![half_day(); 5]
        .into_iter()
        .chain([vec![], vec![]])
        .collect::<Vec<_>>();
    doc.apply(Command::SetCalendarWorkWeek {
        calendar_id: 1,
        name: "Hè".into(),
        from: "2026-06-01".into(),
        to: "2026-08-31".into(),
        week: half,
    })
    .unwrap();
    assert_eq!(doc.project.calendar_by_id(2).unwrap().work_weeks.len(), 1);
    let inherited = doc
        .apply(Command::ClearCalendarWorkWeek {
            calendar_id: 2,
            from: "2026-06-01".into(),
        })
        .unwrap_err();
    assert_eq!(inherited, EngineError::Invalid(Invalid::InheritedFromBase));
    let full = doc.project.calendar_by_id(1).unwrap().week.to_vec();
    doc.apply(Command::SetCalendarWorkWeek {
        calendar_id: 2,
        name: "An làm đủ".into(),
        from: "2026-07-01".into(),
        to: "2026-07-31".into(),
        week: full,
    })
    .unwrap();
    let an = doc.project.calendar_by_id(2).unwrap();
    assert_eq!(
        an.work_weeks.len(),
        1,
        "its own week replaces the overlapped inherited one"
    );
    assert_eq!(an.work_weeks[0].name, "An làm đủ");
    assert_eq!(an.ranges_on(day("2026-07-06")).len(), 2);
}

#[test]
fn a_loop_of_bases_in_a_file_leaves_each_calendar_standing_alone() {
    let doc = with_based_calendar();
    let xml = write_mspdi(&doc.project).unwrap().replacen(
        "<IsBaseCalendar>1</IsBaseCalendar>",
        "<IsBaseCalendar>0</IsBaseCalendar>\n<BaseCalendarUID>2</BaseCalendarUID>",
        1,
    );
    let project = read_mspdi(&xml).unwrap();
    assert!(project
        .calendars
        .iter()
        .all(|calendar| calendar.base.is_none()));
    assert!(project.calendar().has_working_time());
}
