/*
gkvocabdb

Copyright (C) 2021  Jeremy March

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>. 
*/

use super::*;

pub async fn export_text((info, session, req): (web::Query<ExportRequest>, Session, HttpRequest)) -> Result<HttpResponse> {
    let _db = req.app_data::<SqlitePool>().unwrap();
    let bold_glosses = false;

    if let Some(user_id) = login::get_user_id(session) {

        let template = include_str!("latex/doc_template.tex");
        let mut res = template.replace("%BOLDLEMMATA%", if bold_glosses { "\\bf" } else { "" });

        

        Ok(HttpResponse::Ok()
            .content_type("application/x-latex")
            .body(res))
        }
    else {
        let res = ImportResponse {
            success: false,
            words_inserted: 0,
            error: "Export failed: not logged in".to_string(),
        };
        Ok(HttpResponse::Ok().json(res))
    }
}
