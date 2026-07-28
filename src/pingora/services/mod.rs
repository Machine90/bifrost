use pingora::server::Server;

use crate::pingora::services::http_service::HttpService;

pub mod http_service;

pub fn setup(server: &mut Server) {
    server.add_service(HttpService::new());
}
