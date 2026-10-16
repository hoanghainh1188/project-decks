"When will this project actually be done?" A single date on a Gantt chart rarely tells the truth. So I built a tool that answers with odds instead.

Meet Sheafly: a free, open-source project scheduler in the spirit of Microsoft Project, for macOS, Windows and Linux.

Why I built it
Microsoft Project is expensive and Windows-only. Spreadsheet Gantt charts break the moment one task slips. I wanted something that schedules like Project, runs on any desktop, needs no account or server, and speaks both English and Vietnamese properly.

What it does
• Schedules for you: tasks, dependencies, calendars and people in; dates, critical path, costs and overallocations out.
• Levels resources in one click: 5,000 tasks across 500 people in a few dozen milliseconds.
• Plans by real capacity: skills, leave and part-time availability, assignment suggestions you preview before applying.
• Forecasts the finish: give a task a low and high effort ("3 days if it goes well, 8 if not") and Sheafly runs the project 1,000 times to show the chance of finishing by each date, and which tasks put deadlines at risk.
• Tracks progress with baselines, earned value (EVM), burn-down and PDF export.
• Works with what you have: Microsoft Project XML import/export, tasks from Excel or CSV.

A few things I'm quietly proud of
• Everything stays on your machine, in your file.
• Undo history is saved with the project, so you can undo yesterday's change today.
• Every number comes from one engine written in Rust; the interface only shows it, so the same data always gives the same result.

Being honest: the installers aren't signed with an Apple or Microsoft certificate yet (on a Mac, choose "Open Anyway" in Privacy & Security the first time), and .mpp files can't be read; use XML to exchange with Project.

Next up: syncing with Jira, Backlog and Asana, and AI recommendations that propose changes you review before anything moves.

Try it: https://github.com/hoanghainh1188/sheafly/releases

I'd love your feedback, especially if you plan projects for a living or your team lives in Jira, Backlog or Asana. Comments, issues and stars on GitHub are all very welcome.

#ProjectManagement #OpenSource #Rust #Tauri #Scheduling #MonteCarlo #ResourcePlanning
