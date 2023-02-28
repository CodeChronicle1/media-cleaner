mod api;
mod responses;

use std::collections::BTreeMap;

use chrono::prelude::*;
use color_eyre::Result;

use crate::{shared::MediaType, tautulli::responses::ResponseObj};

use self::responses::{History, HistoryItem, HistoryMovie};

#[derive(Debug)]
pub enum WatchHistory {
    Movie(ItemWithHistory<UserMovieWatch>),
    TvShow(ItemWithHistory<UserEpisodeWatch>),
}

impl WatchHistory {
    fn from_user_watches(
        user_watches: BTreeMap<&String, &HistoryItem>,
        media_type: &MediaType,
        rating_key: &str,
    ) -> Self {
        match media_type {
            MediaType::Movie => WatchHistory::create_movie_history(user_watches, rating_key),
            MediaType::Tv => WatchHistory::create_tv_history(user_watches, rating_key),
        }
    }

    fn create_movie_history(
        user_watches: BTreeMap<&String, &HistoryItem>,
        rating_key: &str,
    ) -> Self {
        let watches = user_watches
            .iter()
            .map(|(user, movie_watch)| UserMovieWatch {
                display_name: user.to_string(),
                last_watched: unix_seconds_to_date(movie_watch.date).expect(&format!(
                    "Failed to parse unix time for rating key {}",
                    rating_key
                )),
                progress: movie_watch.percent_complete,
            })
            .collect();

        WatchHistory::Movie(ItemWithHistory {
            rating_key: rating_key.to_string(),
            watches,
        })
    }

    fn create_tv_history(user_watches: BTreeMap<&String, &HistoryItem>, rating_key: &str) -> Self {
        let watches = user_watches
            .iter()
            .map(|(user, tv_watch)| UserEpisodeWatch {
                display_name: user.to_string(),
                last_watched: unix_seconds_to_date(tv_watch.date).expect(&format!(
                    "Failed to parse unix time for rating key {}",
                    rating_key
                )),
                progress: tv_watch.percent_complete,
                season: tv_watch.parent_media_index.unwrap(),
                episode: tv_watch.media_index.unwrap(),
            })
            .collect();

        WatchHistory::TvShow(ItemWithHistory {
            rating_key: rating_key.to_string(),
            watches,
        })
    }
}

#[derive(Debug)]
pub struct ItemWithHistory<T> {
    pub rating_key: String,
    pub watches: Vec<T>,
}

#[derive(Debug)]
pub struct UserEpisodeWatch {
    pub display_name: String,
    pub last_watched: DateTime<Utc>,
    pub progress: u8,
    pub season: u32,
    pub episode: u32,
}

#[derive(Debug)]
pub struct UserMovieWatch {
    pub display_name: String,
    pub last_watched: DateTime<Utc>,
    pub progress: u8,
}

pub async fn get_item_watches(rating_key: &str, media_type: &MediaType) -> Result<WatchHistory> {
    let history = get_item_history(rating_key, media_type).await?;

    let latest_user_history =
        history
            .data
            .iter()
            .fold(BTreeMap::new(), |mut user_latest_watch, current_watch| {
                user_latest_watch
                    .entry(&current_watch.user)
                    .and_modify(|entry: &mut &HistoryItem| {
                        if entry.date < current_watch.date {
                            *entry = current_watch;
                        }
                    })
                    .or_insert(current_watch);

                user_latest_watch
            });

    Ok(WatchHistory::from_user_watches(
        latest_user_history,
        media_type,
        rating_key,
    ))
}

async fn get_item_history(rating_key: &str, media_type: &MediaType) -> Result<History> {
    if let MediaType::Movie = media_type {
        let params = vec![("rating_key".to_string(), rating_key.to_string())];
        let history: ResponseObj<HistoryMovie> = api::get_obj("get_history", Some(params)).await?;
        Ok(history_movie_to_history(history.response.data))
    } else {
        let params = vec![("grandparent_rating_key".to_string(), rating_key.to_string())];
        let history: ResponseObj<History> = api::get_obj("get_history", Some(params)).await?;
        Ok(history.response.data)
    }
}

fn history_movie_to_history(history: HistoryMovie) -> History {
    History {
        draw: history.draw,
        records_total: history.records_total,
        records_filtered: history.records_filtered,
        data: history
            .data
            .into_iter()
            .map(|item| HistoryItem {
                user: item.user,
                date: item.date,
                duration: item.duration,
                percent_complete: item.percent_complete,
                media_index: None,
                parent_media_index: None,
            })
            .collect(),
    }
}

fn unix_seconds_to_date(unix_seconds: i64) -> Option<DateTime<Utc>> {
    let naive_date = NaiveDateTime::from_timestamp_millis(unix_seconds * 1000).unwrap();
    Some(DateTime::from_utc(naive_date, Utc))
}
