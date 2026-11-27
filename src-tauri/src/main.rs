#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::result_large_err)]

fn main() {
    sql_archive_lib::run()
}
