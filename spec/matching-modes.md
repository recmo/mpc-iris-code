# Matching modes

There are four matching modes that need to be supported by the services:

1.  **Enrollment.** On enrollment the iriscode should not already match against the database. A match is both left and right iriscode.
    ```python
    def enrollment(query, threshold):
        for entry in DB:
            dl = distance(query.left, entry.left)
            dr = distance(query.right, entry.right)
            if max(dl, dr) < treshold:
                return Reject
        return Accept
    ```
2.  **Reset.** For a reset there must be exactly one match, which index should be returned. Again using both irises.
    ```python
    def reset(query, threshold):
        matches = []
        for i, entry in enumerate(DB):
            dl = distance(query.left, entry.left)
            dr = distance(query.right, entry.right)
            if max(dl, dr) < treshold:
                matches.append(i)
        if len(matches) == 1:
            return Accept, matches[0]
        else:
            return Reject
    ```
3.  **Orb Authentication.** Here a single match is made to authenticate a user.
    ```python
    def enrollment(query, threshold, index):
        entry = DB[index]
        dl = distance(query.left, entry.left)
        dr = distance(query.right, entry.right)
        if max(dl, dr) < treshold:
            return Accept
        return Reject
    ```
4.  **Local Uniqueness Check.** Here a match is made against a small (say about a thousand) entry subset of the database. The match is now the minimum instead of maximum and the threshold might be adjusted a bit to get lower *false-non-match-rate* at expensive of higher *false-mach-rate* (which is mitigated by the smaller comparison set).
    ```python
    def local_check(query, threshold, indices):
        for entry in DB[indices]:
            dl = distance(query.left, entry.left)
            dr = distance(query.right, entry.right)
            if min(dl, dr) < treshold:
                return Reject
        return Accept
    ```

In addition to this, an **insert** and **delete** is also required to update the database.

Observe that $\max(a, b) < t$ is identical to $(a < t) ∧ (b < t)$ and $\min(a, b) < t$ to $(a < t) ∨ (b < t)$. This allows us to compute the left and right results separately and only combine the boolean results.

Note that the requirements allow us to reveal the individual boolean match results. This allows us to split the services logically into a separate MPC deployment for left and right and do the combination in a cleartext. A single sufficient operation to implement in MPC for mathcing is then:

```python
def matches(query, threshold, indices = All):
    matches = []
    for i, entry in enumerate(DB):
        if i in indices and distance(query, entry) < treshold:
            matches.append(i)
    return matches
```

## Questions

* Does reset need to return the index of the matching record? (If not, how is reset implemented?)
* Does the reject condition in reset distinguish between no-matches and multiple-matches?
* Should enrollment also insert into the database or is this a separate operation?
  * If separate, does the caller guarantee that the uniqueness constraint is upheld?
* Do we want to do filtering for V2?
* Can we assume no meaningful information leakage from revealing individual boolean match results?
* Can we specify a 'safe range' for treshold to avoid information leakage?
* Can we also constrain the set of indices, e.g. one thousand out of the last hundred-thousand inserted.
* What is the distribution of sizes of the `matches` set for individual eyes?
* Do we need to check that masks have sufficient bits available in the intersection?
* Can we and do we want to do filtering for V2?
* Are secret masks a requirement?
