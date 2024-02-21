# Version 2 requirements

## Interface

1.  **Matches.** Returns the indices of matching records given a query and a threshold. See [matching-modes.md](./matching-modes.md) for why this covers all use-cases.
    ```python
    def matches(query, threshold, indices = All):
        matches = []
        for i, entry in enumerate(DB):
            if i in indices and distance(query, entry) < threshold:
                matches.append(i)
        return matches, len(DB)
    ```
    The values `DB` and `query` are secret, but the `threshold`, `indices`, `matches` and size of the database are cleartext.

    To prevent unwanted information leakage the parties can require `threshold` to be in an acceptable set of values and also put constraints on `indices`, for example at most a thousand entries from the last hundred thousand insertions.

    As each insert monotonically increments `len(DB)`, it functions as a database version counter.

    **Q** Do we check that masks have sufficient bits available in the intersection?

2.  **Insert.** Add a new entry to the database and return the index.
    ```python
    def insert(entries, start_index):
        assert start_index <= len(DB)
        DB.append(entries[len(DB) - start_index:])
        return len(DB)
    ```

    The `entry` and `DB` are secret, but no MPC operation should be required to implement this. The `start_index` serves to make the batch insert operation idempotent. (**Q** Should we check that the overlapping range are identical?)

    **Q** Do we want to re-randomize the entry on insertion?

3.  **Delete.**
    ```python
    def delete(index):
        DB[index] = None
        return Success
    ```

    The `delete` operation does not change the order of the indices, but it does irreversibly delete the data at the provided index. Ideally it also prevents the record from ever being returned again, but this can also be done in cleartext post-processing.

## Necessary properties

Necessary:

* Secret iriscode bits in DB.
* Secret iriscode bits in Query.
* Secret distances.
* At least two parties.
* Practically scalable to 5M DB entries and 10 queries/second on AWS.
* E2E encryption. PKI.
* Semi-honest security through MPC.
* Malicious secure through trusted execution environment.

Desirable:

* Secret masks.
* Practically scalable to 50M DB entries and 100 queries/second.
* Scalable through more parties.
* Malicious secure through MPC.

Acceptable:

* Trusted dealer role.
* Preprocessing of inputs.

## Questions

* Do we check that masks have sufficient bits available in the intersection?
* Should we check that the overlapping range on insert are identical?
  * This should be a cheap operation assuming they are not re-randomized.
* Do we want to do filtering for V2?
* We assume no meaningful information leakage from match results.
* Are secret masks a requirement?

## Performance considerations

In v1 the database fits in memory and the bottleneck is memory bandwidth. With this in mind we should aim to keep the database (shard) such that it can fit in memory or GPU.

To make best use of memory bandwidth we can do batch processing of multiple queries, so we handle multiple requests for each linear scan through the database. The ideal batch size will be a function of the cache hierarchy.

To make best use of the cache hierarchy we can use non-cached fetches for the database entries.

Each match can be computed individually, and these results can be treated as a stream. This allows a pipelined approach to get maximal throughput regardless of network latency.

* A size/compute vs bandwidth tradeoff is to accumulate partial popcounts in $ℤ_8$ and lift these to $ℤ_{16}$ before adding them. This requires about $12,800 / 256 = 50$ partial sums. This does not seem worthwhile.
