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

use actix_session::Session;
use actix_web::http::header::ContentType;
use actix_web::http::header::LOCATION;
use actix_web::web;
use actix_web::Error as AWError;
use actix_web::HttpRequest;
use actix_web::HttpResponse;
use secrecy::Secret;

use gkvocabdb::dbsqlite::GlosserDbSqlite;
use gkvocabdb::gkv_validate_credentials;
use gkvocabdb::Credentials;

use crate::map_glosser_error;

#[derive(serde::Deserialize)]
pub struct LoginFormData {
    username: String,
    password: Secret<String>,
}

pub fn get_user_id(session: Session) -> Option<u32> {
    session.get::<u32>("user_id").unwrap_or(None)
}

pub async fn logout(session: Session) -> Result<HttpResponse, AWError> {
    session.purge();
    //FlashMessage::error(String::from("Authentication error")).send();
    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/login"))
        .finish())
}

pub async fn login_get() -> Result<HttpResponse, AWError> {
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(r#"<!DOCTYPE html>
<html lang="en">
    <head>
        <meta http-equiv="content-type" content="text/html; charset=utf-8">
        <title>Login</title>
        <script>
            function setTheme() {
                var mode = localStorage.getItem("mode");
                if ((window.matchMedia( "(prefers-color-scheme: dark)" ).matches || mode == "dark") && mode != "light") {
                    document.querySelector("HTML").classList.add("dark");
                }
                else {
                    document.querySelector("HTML").classList.remove("dark");
                }
            }
            setTheme();
        </script>
        <style>
            BODY { font-family:helvetica;arial;display: flex;align-items: center;justify-content: center;height: 87vh; }
            TABLE { border:2px solid black;padding: 24px;border-radius: 10px; }
            BUTTON { padding: 3px 16px; }
            .dark BODY { background-color:black;color:white; }
            .dark INPUT { background-color:black;color:white;border: 2px solid white;border-radius: 6px; }
            .dark TABLE { border:2px solid white; }
            .dark BUTTON { background-color:black;color:white;border:1px solid white; }
        </style>
    </head>
    <body>
        <form action="/login" method="post">
            <table>
                <tbody>
                    <tr>
                        <td>               
                            <label for="username">Username</label>
                        </td>
                        <td>
                            <input type="text" id="username" name="username">
                        </td>
                    </tr>
                    <tr>
                        <td>
                            <label for="password">Password</label>
                        </td>
                        <td>
                            <input type="password" id="password" name="password">
                        </td>
                    </tr>
                    <tr>
                        <td colspan="2" align="center">
                            <button type="submit">Login</button>
                        </td>
                    </tr>
                </tbody>
            </table>
        </form>
        <script>/*document.getElementById("username").focus();*/</script>
    </body>
</html>"#))
}

pub async fn login_post(
    (session, form, req): (Session, web::Form<LoginFormData>, HttpRequest),
) -> Result<HttpResponse, AWError> {
    let db = req.app_data::<GlosserDbSqlite>().unwrap();

    let credentials = Credentials {
        username: form.0.username,
        password: form.0.password,
    };

    if let Ok(user_id) = gkv_validate_credentials(db, credentials)
        .await
        .map_err(map_glosser_error)
    {
        session.renew(); //https://www.lpalmieri.com/posts/session-based-authentication-in-rust/#4-5-2-session
        if session.insert("user_id", user_id).is_ok() {
            return Ok(HttpResponse::SeeOther()
                .insert_header((LOCATION, "/"))
                .finish());
        }
    }

    session.purge();
    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/login"))
        .finish())
}
