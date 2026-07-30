pub mod alarm;
pub mod notification;
pub mod online;

use rocket_db_pools::Database;
use rocket_db_pools::deadpool_redis::Pool;

#[derive(Database)]
#[database("online_redis_pool")]
pub struct OnlineRedisPool(Pool);

#[derive(Database)]
#[database("notifications_redis_pool")]
pub struct NotificationsRedisPool(Pool);

#[derive(Database)]
#[database("alarms_redis_pool")]
pub struct AlarmsRedisPool(Pool);
