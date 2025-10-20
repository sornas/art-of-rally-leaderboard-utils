use art_of_rally_leaderboard_api::Platform;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub webhook_url: String,
    pub platform: Platform,
    users: Vec<User>,
}

#[derive(Deserialize)]
pub struct User {
    id: u64,
    name: String,
}

impl Config {
    pub fn users(&self) -> (Vec<u64>, Vec<&str>) {
        let mut user_ids = Vec::new();
        let mut user_names = Vec::new();
        for user in &self.users {
            user_ids.push(user.id);
            user_names.push(user.name.as_str());
        }
        (user_ids, user_names)
    }
}
