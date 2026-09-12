---
"takumi": minor
---

# Keep decoded photos in the resource cache

The resource cache split its budget across one shard per four CPU threads and refused any entry larger than a shard, so on a 12-core machine a decoded image over about 250 KB was never kept and was decoded again on every render. Shards now hold 64 MiB each, and the default `cacheMaxBytes` rises from 16 MiB to 64 MiB, so a 2400 × 1600 photo stays cached.
