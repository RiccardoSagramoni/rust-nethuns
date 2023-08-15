# Single-Producer Single-Consumer Queue

In order to have an optimized, cache-friendly, lock-free Single-Producer Single-Consumer (SPSC) queue, use [`rtbr`](https://crates.io/crates/rtrb) crate.

## Features comparison

|   Nethuns                     |   rtbr                |
| - | - |
|   nethuns_spsc_init           |   RingBuffer::new     |
|   nethuns_slot_addr           |   *not required for public API*   |
|   nethuns_spsc_is_empty       |   Consumer::is_empty  |
|   nethuns_spsc_is_full        |   Producer::is_full   |
|   nethuns_spsc_distance       |   *private RingBuffer::distance*  |
|   nethuns_spsc_consumer_sync  |   *automatically handled* |
|   nethuns_spsc_producer_sync  |   *automatically handled* |
|   nethuns_spsc_len            |   Consumer::slots     |
|   nethuns_spsc_next_index     |   *private RingBuffer::increment1*    |
|   nethuns_spsc_push           |   Consumer::push      |
|   nethuns_spsc_pop            |   Consumer::pop       |
|   nethuns_spsc_peek           |   Consumer::peek      |
|   nethuns_consume             |   equivalent to `consumer.read_chunk(consumer.slots).unwrap().commit_all()` |
|   nethuns_spsc_free           |   *implemented in Drop trait*     |
