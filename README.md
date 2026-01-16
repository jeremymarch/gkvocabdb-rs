### Glosser

## Installing and running

- if not installed, install rust:
    - curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

- if not set, set environment variables for sqlite db and session key. e.g.:
    - export GKVOCABDB_DB_PATH=sqlite://gkvocabnew.sqlite?mode=rwc
    - export GKVOCABDB_KEY=56d520157194bdab7aec18755508bf6d063be7a203ddb61ebaa203eb1335c2ab3c13ecba7fc548f4563ac1d6af0b94e6720377228230f210ac51707389bf3285

- build:
    - cargo run

- Open web browser to http://0.0.0.0:8088




## To do:
- Text Nesting (I think we only need to support one level of nesting)
  - Changing order of nested texts is broken: need to account for 1) texts with children (move children with parent) and 2) texts with parents (only move them up and down within parent)
  - Add left and right arrow buttons to text list to nest/unnest texts
- Courses
  - Implement logging in to different courses, currently we can only used course 1
  - Create a way to add existing texts to a course or remove a text from a course
  - Allow Latin texts/courses
- Exporting texts
  - re-implement exporting to LaTeX
- Importing texts
  - Progress indicator for changing order of texts since it takes a while?
- UI
  - add button to toggle on/off the Greek keyboard
- Infrastructure
  - move logins to db as opposed to usernames/passwords hardcoded
  - add unit tests for more features
  - improve the update log, it is pretty crappy right now
    - probably overly difficult: update log can rollback changes to previous verions
  - clean up db schema: there are a lot of columns which are no longer used and can be removed
  - improve the error messages passed back to browser if something goes wrong


  az login
  az acr login -n philologuscontainerregistry
  docker build --load --builder multi-platform-builder --platform=linux/amd64 -t gkvocabdb:0.1.5 .
  docker tag gkvocabdb:0.1.5 philologuscontainerregistry.azurecr.io/gkvocabdb:0.1.5
  docker push philologuscontainerregistry.azurecr.io/gkvocabdb:0.1.5

pg_dump -h philologuspostgresdb.postgres.database.azure.com -p 5432 -U philuserdb -f pgdump-gkvocabdb-bk1.sql gkvocabdb
psql -h philologuspostgresdb.postgres.database.azure.com -p 5432 -U philuserdb gkvocabdb
