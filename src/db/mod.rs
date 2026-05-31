pub mod notification;
pub mod online;

use rocket_db_pools::Database;
use rocket_db_pools::deadpool_redis::Pool;

#[derive(Database)]
#[database("redis_pool")]
pub struct RedisPool(Pool);

#[derive(Database)]
#[database("notifications_redis_pool")]
pub struct NotificationsRedisPool(Pool);
