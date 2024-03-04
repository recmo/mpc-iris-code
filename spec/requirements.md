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

* Secret iris code bits and masks in Query and DB.
* Secret distances.
* At least two parties required for collusion.
* Practically scalable to 10M DB entries and 10 queries/second on AWS.
* E2E encryption. PKI.
* Semi-honest security through MPC.
* Malicious secure through trusted execution environment.

Desirable:

* Practically scalable to 50M DB entries and 100 queries/second.
* Scalable through more parties, but works with two.
* Malicious secure through MPC.

Acceptable:

* Revealing which rotation matched.
* More than two parties.
* Trusted dealer role.
* Preprocessing of inputs.

## Objective function

The main metric is total operating cost 

## Questions

* Do we check that masks have sufficient bits available in the intersection?
* Should we check that the overlapping range on insert are identical?
  * This should be a cheap operation assuming they are not re-randomized.
* We assume no meaningful information leakage from match results.
* Are secret masks a requirement?

## Performance considerations

In v1 the database fits in memory and the bottleneck is memory bandwidth. With this in mind we should aim to keep the database (shard) such that it can fit in memory or GPU.

To make best use of memory bandwidth we can do batch processing of multiple queries, so we handle multiple requests for each linear scan through the database. The ideal batch size will be a function of the cache hierarchy.

To make best use of the cache hierarchy we can use non-cached fetches for the database entries.

Each match can be computed individually, and these results can be treated as a stream. This allows a pipelined approach to get maximal throughput regardless of network latency.

* A size/compute vs bandwidth tradeoff is to accumulate partial popcounts in $ℤ_8$ and lift these to $ℤ_{16}$ before adding them. This requires about $12,800 / 256 = 50$ partial sums. This does not seem worthwhile.

---

Comments on the Markdown:
Re Reset: For monitoring it would be good to return all matches, regardless of Accept or Reject.
Generally I would make the threshold dynamic for each mode or maybe even each individual call.
Until now we are only considering the case of min(dl, dr) and max(dl, dr) which is great because we can use & and | and see “left” and “right” as detached systems that both only need to report a boolean value. However, we might want to experiment with other functions in the future e.g. mean(min(dl,0.4), min(dr,0.4)). Expressions like this might help us compensate bad image quality that usually lead to higher HDs for matching pairs. He have never tested this extensively and this might only be a long term optimisation, but worth to keeping in mind.
Questions:
Does reset need to return the index of the matching record? (If not, how is reset implemented?)
Yes, we need to know index, otherwise we do not know which idComm to replace.
Does the reject condition in reset distinguish between no-matches and multiple-matches?
Yes it does. For now, the plan is show “Please contact support” in the app for the case of multple matches so that we can investigate what actually happened. For the case of no matches, we would ask the user to enroll since the user either has not yet signued up, or his or her template was deleted in a clean up last year.
Should enrollment also insert into the database or is this a separate operation?
Not sure if I get the question right. From a product persepective yes. At least I do not see an use-case for just asking “Do I match with someone in the DB”. However, treating it internally as two distinct processes might make sense to keep it flexible for the future. But no strong opinion here.
Do we want to do filtering for V2?
Afaik it is a must have. With the growth projections of 10M user by June and 40M by EoY the service would cost us ~1M per month by EoY. Filtering will give us a ~60x speed up, which probably translates to roughly the some reduction of cost?!
Can we assume no meaningful information leakage from revealing individual boolean match results?
I am no expert on this one. That is a question I would usually ask you :slightly_smiling_face:
Can we specify a ‘safe range’ for treshold to avoid information leakage?
I think we have a quite good understanding of the non-match distribution (if I remember correctly we can model it as a skewed Bernoulli distribution). This might help us the at least to assign some probability on whether the resulting HD of a pair comes from a Bernoulli distribution, which would mean it is “somewhat random”. But that is just an idea, I have never thought about that more thoroughly.
Can we also constrain the set of indices, e.g. one thousand out of the last hundred-thousand inserted.
Not sure if I get this question right. That is what we are doing in the Local Uniqueness Service.
What is the distribution of sizes of the matches set for individual eyes?
What do you mean by ’sizes of matches`?
Do we need to check that masks have sufficient bits available in the intersection?
Yes, that is part of the QA that already happens on the orb. If there are two many bits masked out we reject the signup right aways. However, there is also a way to accounting for the size of the mutual mask. See here, section “Score Normalization”. We do not yet have that deployed but the team is experimenting with it. Not sure if we ever need it, though.
Are secret masks a requirement?
That is a legal question the BayLDA needs to answer. IMO no, but that does not count here.



