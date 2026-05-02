use axum::{
    routing::{get, patch, post, put, delete},
    Router,
};
use std::sync::Arc;
use worker::Env;

use crate::handlers::{auth, bangumi, dashboard, friend, gallery, health, legal, like, link, post, proxy, search, settings, status, trash};

pub fn api_router(state: Arc<Env>) -> Router {
    Router::new()
        .route("/api/health", get(health::health_check))
        .route("/api/status", get(status::site_status))
        // 身份验证
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/login/2fa", post(auth::login_2fa))
        .route("/api/auth/login/verify", post(auth::login_verify))
        .route("/api/auth/2fa/setup", post(auth::setup_2fa))
        .route("/api/auth/2fa/verify", post(auth::verify_2fa))
        .route("/api/auth/2fa/disable", post(auth::disable_2fa))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/refresh", post(auth::refresh))
        .route("/api/auth/dashboard", get(dashboard::dashboard))
        // 站点设置
        .route("/api/settings/profile", get(settings::get_profile).put(settings::update_profile))
        .route("/api/settings/footer", get(settings::get_footer).put(settings::update_footer))
        .route("/api/settings/footer-links", get(settings::list_footer_links).post(settings::create_footer_link))
        .route("/api/settings/footer-links/{id}", put(settings::update_footer_link).delete(settings::delete_footer_link))
        .route("/api/settings/social-links", get(settings::list_social_links).post(settings::create_social_link))
        .route("/api/settings/social-links/{id}", put(settings::update_social_link).delete(settings::delete_social_link))
        .route("/api/settings/announcements", get(settings::list_announcements).post(settings::create_announcement))
        .route("/api/settings/announcements/{id}", put(settings::update_announcement).delete(settings::delete_announcement))
        // 博客模块
        .route("/api/posts", get(post::list_posts).post(post::create_post))
        .route("/api/posts/{slug}", get(post::get_post).put(post::update_post).delete(post::delete_post))
        .route("/api/posts/{slug}/views", post(post::increment_views))
        .route("/api/posts/{slug}/comments-count", get(post::comments_count))
        // 全站搜索
        .route("/api/search", get(search::search_content))
        // 点赞系统
        .route("/api/likes/{target_type}/{target_id}", get(like::get_likes).post(like::toggle_like))
        // 友人帐
        .route("/api/friends", get(friend::list_friends).post(friend::create_friend))
        .route("/api/friends/{id}", get(friend::get_friend).put(friend::update_friend).delete(friend::delete_friend))
        .route("/api/friends/{id}/status", patch(friend::review_friend))
        // 链接收藏
        .route("/api/links", get(link::list_links).post(link::create_link))
        .route("/api/links/meta", get(link::get_link_meta))
        .route("/api/links/{id}", get(link::get_link).put(link::update_link).delete(link::delete_link))
        // 相册
        .route("/api/galleries", get(gallery::list_galleries).post(gallery::create_gallery))
        .route("/api/galleries/{id}", get(gallery::get_gallery).put(gallery::update_gallery).delete(gallery::delete_gallery))
        .route("/api/galleries/{id}/photos", post(gallery::add_photos))
        .route("/api/photos/{id}", delete(gallery::delete_photo))
        // 法律文档
        .route("/api/license", get(legal::get_license).post(legal::create_license))
        .route("/api/license/versions", get(legal::list_license_versions))
        .route("/api/privacy", get(legal::get_privacy).post(legal::create_privacy))
        .route("/api/privacy/versions", get(legal::list_privacy_versions))
        // 番剧记录
        .route("/api/bangumi/records", get(bangumi::list_records).post(bangumi::create_record))
        .route("/api/bangumi/records/{id}", put(bangumi::update_record).delete(bangumi::delete_record))
        // 外部 API 代理
        .route("/api/bangumi/search", post(proxy::bangumi_search))
        .route("/api/bangumi/subjects/{id}", get(proxy::bangumi_subject))
        .route("/api/bangumi/calendar", get(proxy::bangumi_calendar))
        .route("/api/bangumi/browse", get(proxy::bangumi_browse))
        .route("/api/anime-garden/resources", get(proxy::anime_garden_resources))
        // 翻译
        .route("/api/translate", post(proxy::translate))
        // 垃圾桶
        .route("/api/trash/{resource}", get(trash::list_trash))
        .route("/api/trash/{resource}/{id}", delete(trash::hard_delete))
        .route("/api/trash/{resource}/{id}/restore", post(trash::restore))
        .with_state(state)
}
