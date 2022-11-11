### gkvocabdb

if not installed, install rust:
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

if not set, set environment variable for sqlite db and session key. e.g.: 
    export GKVOCABDB_DB_PATH=sqlite://gkvocabnew.sqlite?mode=rwc
    export GKVOCABDB_KEY=56d520157194bdab7aec18755508bf6d063be7a203ddb61ebaa203eb1335c2ab3c13ecba7fc548f4563ac1d6af0b94e6720377228230f210ac51707389bf3285

build:
    cargo run

Open web browser to http://0.0.0.0:8088



## To do:
- Running count and total count are not updated on nested texts when changing order
- Updating running count and total count is pretty inefficient when changing order
- Progress indicator for changing order of texts since it takes a while?
- Implement exporting to LaTeX
- add left and right arrows to text list to nest/unnest texts
- add button to toggle on/off the Greek keyboard
- Implement logging in to different courses, currently we can only used course 1
- Allow Latin texts/courses
- move logins to db as opposed to usernames/passwords hardcoded
- add unit tests for more features
- improve the update log, it is pretty crappy right now
- probably overly difficult: update log can rollback changes to previous verions
