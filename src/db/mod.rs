pub mod online;

use rocket_db_pools::deadpool_redis::Pool;
use rocket_db_pools::Database;

#[derive(Database)]
#[database("redis_pool")]
pub struct RedisPool(Pool);
