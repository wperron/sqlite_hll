# SQLite HyperLogLog

Compiles a dynamic library that adds an `approx_count_distinct` built on HyperLogLog.
This gives better performance than a traditional `count(distinct column)` at the cost of precision.
The error rate is set at 0.81% for no other reason than it being the [default value used by Redis][1].

> ⚠️ NOTE: Use this at your own risk, I make no commitment to maintain or update this library. ⚠️

[1]: https://redis.io/docs/data-types/probabilistic/hyperloglogs/
