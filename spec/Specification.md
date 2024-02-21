# Specification

$$
\def\vec#1{\mathbf{#1}}
\def\T{\mathsf{T}}
\def\F{\mathsf{F}}
\def\U{\mathsf{U}}
\def\popcount{\mathtt{popcount}}
\def\count{\mathtt{count}}
\def\fhd{\mathtt{fhd}}
\def\vsum{\mathtt{sum}}
$$

First we need some definitions and theory on masked bitvectors and their representation in $ℤ_{2^{16}}$ and present a simple secure multiparty computation scheme.

## Masked bitvectors

### Binary operations in rings

Given a bitvector $\vec b ∈ \{\F,\T\}^n$ of length $n$, we take the usual binary operations of *not* $¬$, *and* $∧$, *or* $∨$, *xor* $⊕$, and also $\popcount$. We can embed this in a ring $R$ by representing $\F,\T$ as $0,1$ respectively and using the following operations on vectors $\vec b ∈ R^n$:

$$
\begin{aligned}
¬ \vec a &= 1 - \vec a \\
\vec a ∧ \vec b &= \vec a ⊙ \vec b \\
\vec a ∨ \vec b &= \vec a + \vec b - \vec a ⊙ \vec b \\
\vec a ⊕ \vec b &= \vec a + \vec b - 2 ⋅ \vec a ⊙ \vec b \\
\popcount(\vec a) &= \vsum(\vec a) \\
\end{aligned}
$$

Where $⊙$ is the element wise product and $\vsum$ adds the vector elements. Note that $\vsum(\vec a ⊙ \vec b)$ is the vector dot product $\vec a ⋅ \vec b$. Note also that the $\popcount$ will be computed in the ring, so a sufficiently large modular ring is required.

### Masked binary operations in rings

A *masked bitvector* has three values $\mathtt{b} ∈ \{\F, \T, \U\}^n$ which should be interpreted as *false*, *true* and *unavailable* respectively. The operations are the same as for binary, except when one of the arguments is $\U$, the result is always $\U$. We take $\popcount$ to count the number of $\T$'s and introduce $\mathtt{count}$ to count the number of available entries i.e. either $\F$ or $\T$ but not $\U$.

We can represent this on a suitably large (**Q** how large?) ring as $-1,0,1$ for $\F, \U, \T$ respectively.

$$
\begin{aligned}
¬ \vec a &= - \vec a \\
\vec a ∧ \vec b &= ½ ⋅ \vec a ⊙ \vec b ⊙ (1 + \vec a + \vec b - \vec a ⊙ \vec b) \\
\vec a ∨ \vec b &= ½ ⋅ \vec a ⊙ \vec b ⊙ (\vec a ⊙ \vec b + \vec a + \vec b -1 ) \\
\vec a ⊕ \vec b &= -\vec a ⊙ \vec b \\
\count(\vec a) &= \vsum\left(\vec a^{⊙2}\right) \\
\popcount(\vec a) &= ½ ⋅ \vsum\left(\vec a^{⊙2} + \vec a\right)
\end{aligned}
$$

Note that squaring, $\vec a^{⊙2}$, produces a regular $0,1$ bitvector that is $0$ whenever the value is $\U$ and $1$ otherwise, i.e. it  allows us to extract the mask as a regular bitvector. Similarly $½ ⋅(\vec a^{⊙2} + \vec a)$ extracts the data bits: $1$ for $\T$ and $0$ otherwise. The reverse mapping, converting data bits $\vec b$ and mask bits $\vec m$ to a masked bitvector is $\vec m - 2⋅\vec b⊙\vec m$. If the data bits are known to be $\F$ in the in the unavailable region, then it simplifies to $\vec m - 2⋅\vec b⊙\vec m$.

In this representation *and* and *or* have awkward fourth degree expressions, though they can be evaluated using only two multiplies. The expressions for *xor*, $\count$ and $\popcount$ are quite nice considering that they correctly account for masks. This makes it a suitable system for computing fractional hamming distances.

### Fractional hamming distance

The *fractional hamming weight* of a masked bitvector $\vec a$ is defined as

$$
\begin{aligned}
\mathtt{fhw}(\vec a) &= \frac
{\popcount(\vec a)}
{\count(\vec a )}
\end{aligned}
$$

and the *fractional hamming distance* between two masked bitvectors $\vec a$ and $\vec b$ as

$$
\begin{aligned}
\fhd(\vec a, \vec b)
&= \mathtt{fhw}(\vec a ⊕ \vec b) = \frac
{\popcount(\vec a ⊕ \vec b)}
{\count(\vec a ⊕ \vec b)}
\end{aligned}
$$

In the ring representation these can be computed as

$$
\begin{aligned}
\fhd(\vec a, \vec b)
& =\frac
{ ½ ⋅ \vsum\left((-\vec a ⊙ \vec b)^{⊙2} -\vec a ⊙ \vec b\right)}
{\vsum\left((-\vec a ⊙ \vec b)^{⊙2}\right)} 
&&=\frac{1}{2} -\frac
{ \vsum\left(\vec a ⊙ \vec b\right)}
{2⋅\vsum\left((\vec a ⊙ \vec b)^{⊙2}\right)}  \\
\end{aligned}
$$

Note that $(\vec a ⊙ \vec b)^{⊙2} = \vec a^{⊙2} ⊙ \vec b^{⊙2}$ and thus depends only on the masks of $\vec a$ and $\vec b$. Given the masks $\vec a_m$ and $\vec b_m$ it can be computed in binary as

$$
\vsum\left((\vec a ⊙ \vec b)^{⊙2}\right) = \popcount(\vec a_m ∧ \vec b_m)
$$

## Secure Multiparty Computation

We construct a simple secret sharing scheme over $ℤ_{2^{16}}$. To encrypt a value $x$ we compute $n$ random secret shares $x_0,…,x_{n-1}$ such that

$$
x = \sum_{i∈[0,n)} x_i
$$

One way to do this is by generating $n-1$ random values $x_0,…,x_{n-2}∈ℤ_{2^{16}}$ and solving for $x_{n-1}$:

$$
x_{n-1} = x - \sum_{i∈[0,n-1)} x_i
$$

The shares are encrypted in the sense that this is essentially a [one-time-pad] with any $n-1$ shares being the decryption key for the remaining share. Another valid perspective is that the shares are independently random and the secret is stored in their correlation.

[one-time-pad]: https://en.wikipedia.org/wiki/One-time_pad

### Additive homomorphism

This secret sharing scheme is additive homomorphic. Given two secrets $a$ and $b$ with shares $a_i$ and $b_i$, each party can locally compute $c_i = a_i + b_i$. These $c_i$ are valid shares for the sum $c = a + b$.

Similarly we can compute the product of a secret $a$ with shares $a_i$ and a constant $b ∈ ℤ_{2^{16}}$ locally as $c_i = a_i ⋅ b$.

Generalizing this, given a secret vector $\vec a ∈ ℤ_{2^{16}}^n$ and a cleartext vector $\vec b ∈ ℤ_{2^{16}}^n$ we can compute shares of the dot product $\vec c = \vec a ⋅ \vec b$ as $c_i = \sum_j a_{ij} \cdot b_j$.

Note that these newly created shares are correlated with the input secret shares. To solve this, they need to be re-randomized.

### Re-randomizing

To re-randomize results we construct secret shares of zero and add them to the result. One way to do this locally is for each party to have two random number generators, $r_0, …, r_{n-1}$ and some cyclic permutation $r_{σ(0)}, … r_{σ(n-1)}$. The a secret share $a_i$ is locally updated as:

$$
a_i' = a_i + r_i - r_{σ(i)}
$$

One method for parties $i$ and $σ(i)$ to establish shared randomness is by using [Diffie-Hellman][DH] to establish a seed for a cryptographic PRNG.

[DH]: https://en.wikipedia.org/wiki/Diffie%E2%80%93Hellman_key_exchange

## Iris codes

For present purposes an iriscode is a $12\ 800$-bit masked bitvector.

### Rotations

The $12\ 800$ masked bitvectors can be interpreted as $64 × 200$ bit matrices. We can then define a rotation as a permutation on the columns:

$$
\mathtt{rot}(\vec b, n)[i,j] = \vec b[i,(j+n)\ \mathrm{mod}\ 200]
$$

### Distance

The *distance* between two iriscodes $\mathtt a$ and $\mathtt b$ is defined as the minimum distance over rotations from $-15$ to $15$:

$$
\mathtt{dist}(\vec a, \vec b) = \min_{r∈[-15,15]}\ \fhd(\mathtt{rot}(\vec a, r), \vec b)
$$

### Uniqueness

To verify uniqueness we require that an iriscode $\vec a$ is a certain minimum distance from all previous iriscodes:

$$
\mathop{\Large ∀}\limits_{\vec b ∈ \mathtt{DB}}\ 
\mathtt{dist}(\vec a, \vec b) > \mathrm{threshold}
$$

where $\mathtt{DB}$ is the set of already registered iriscodes (currently 3 million entries).

When there is a match, we are also interested in finding the location of the best match. Both can be addressed by implementing a method that returns the index of the minimum distance entry.

## Iriscode SMPC v2

Objective: iriscodes, masks and distances always encrypted. Threshold plaintext.

Observe that 

$$
\mathrm{threshold}
≥ \mathtt{dist}(\vec a, \vec b) 
= \min_{r∈[-15,15]}\ \fhd(\mathtt{rot}(\vec a, r), \vec b)
$$

is the same as the 'or' of the 31 individual checks.

$$
\bigvee_{r∈[-15,15]}\ \mathrm{threshold}
≥\fhd(\mathtt{rot}(\vec a, r), \vec b)
$$

$$
\mathrm{threshold} ≥\fhd(\mathtt{rot}(\vec a, r), \vec b)
$$

Iriscodes are shared as masked bitvectors.

The threshold check can be converted to a sign-bit check if we have an approximation of $\mathrm{threshold}$ as a fraction $\frac ab$.

$$
\begin{aligned}
\mathrm{threshold} &≥\fhd(\mathtt{rot}(\vec a, r), \vec b) \\
\frac{a}{b} &≥ \frac
{1 - \vsum\left(\vec a ⊙ \vec b\right)}
{2⋅\vsum\left((\vec a ⊙ \vec b)^{⊙2}\right)}  \\
2 ⋅ a ⋅ \vsum\left((\vec a ⊙ \vec b)^{⊙2}\right)
&≥ b - b ⋅ \vsum\left(\vec a ⊙ \vec b\right) \\
0 &≥  b
- b ⋅ \vsum\left(\vec a ⊙ \vec b\right) 
- 2 ⋅ a ⋅ \vsum\left((\vec a ⊙ \vec b)^{⊙2}\right) \\
\end{aligned}
$$

Call the expression on the rhs $f_{a,b}(\vec a, \vec b)$:

$$
f_{a,b}(\vec a, \vec b) = b
- b ⋅ \vsum\left(\vec a ⊙ \vec b\right)
- 2 ⋅ a ⋅ \vsum\left((\vec a ⊙ \vec b)^{⊙2}\right)
$$

Computing $(\vec a ⊙ \vec b)^{⊙2}$ requires two consecutive multiply operations, which would mean an interactive protocol. This computations is just $\count(\vec a ⊕ \vec b)$ and can be computed directly from the masks as

$$
\vsum\left(\vec a_{\mathrm m} ⊙ \vec b_{\mathrm m}\right)
$$

where $\vec a_{\mathrm m}$ is the bitvector representing the mask of $\vec a$. These can be stored in a separate database. This doubles the storage (and memory bandwidth) requirements, but reduces the communication complexity by $12,800×$, which seems a worthwhile tradeoff.

We can then compute an non-replicated secret share of $[f(\vec a, \vec b)]_{\mathrm{a}}$ locally, given replicated shares $[\vec a]_{\mathrm r}$, $[\vec a_{\mathrm m}]_{\mathrm r}$, $[\vec b]_{\mathrm r}$, $[\vec b_{\mathrm m}]_{\mathrm r}$ by using the linearity:

$$
[f_{a,b}(\vec a, \vec b)]_{\mathrm{a}} =
b
- b ⋅ \vsum\left([\vec a]_{\mathrm r} ⊙ [\vec b]_{\mathrm r}\right)
- 2 ⋅ a ⋅ \vsum\left([\vec a_{\mathrm m}]_{\mathrm r} ⊙ [\vec b_{\mathrm m}]_{\mathrm r}\right)
$$

The threshold constants can be mostly pushed into the query $\vec a$. Together with a secret $[b]_{\mathrm a}$ this allows a secret threshold value:

$$
[f_{a,b}(\vec a, \vec b)]_{\mathrm{a}} =
[b]_{\mathrm a}
- \vsum\left([b ⋅ \vec a]_{\mathrm r} ⊙ [\vec b]_{\mathrm r}\right)
-\vsum\left([2 ⋅ a ⋅ \vec a_{\mathrm m}]_{\mathrm r} ⊙ [\vec b_{\mathrm m}]_{\mathrm r}\right)
$$

Another interesting observation is the near-simplification due to Bryan:

$$
\begin{aligned}
&\vsum([b ⋅ \vec a + 2 ⋅ a ⋅ \vec a_{\mathrm m}]_{\mathrm r} ⊙ [\vec b + \vec b_{\mathrm m}]_{\mathrm r}) \\
&=
\vsum([b ⋅ \vec a]_{\mathrm r} ⊙ [\vec b ]_{\mathrm r}) +\vsum([2 ⋅ a ⋅ \vec a_{\mathrm m}]_{\mathrm r}) ⊙ [\vec b_{\mathrm m}]_{\mathrm r}) \\
&+ \vsum([b ⋅ \vec a]_{\mathrm r} ⊙ [\vec b_{\mathrm m}]_{\mathrm r}) +\vsum([2 ⋅ a ⋅ \vec a_{\mathrm m}]_{\mathrm r}) ⊙ [\vec b]_{\mathrm r}) \\
\end{aligned}
$$

The cross-terms here are noisy, but if the iris code are centered in that their expected value is zero, then both these cross terms also have an expected value of zero. With this trick the performance is essentially the same as in v0.

After $[f]_{\mathrm a}$ is computed we need to compute it's sign. For this it needs to be evaluated in a ring large enough that the minimum possible value and maximum possible value do not overlap.

#### Extracting the sign bit

We need a function that splits the ring into two contiguous ranges $[a..b)$ and $[b..a)$ (wrapping around the modulus as needed), such that $f(x)$ is either of two values depending on which range $x$ falls in.

##### Algebraically

**Theorem.** The $\mathrm{sign}$ function, that takes the most significant bit and exposes it through the least significant bit, is not polynomial in $ℤ_{2^k}$ for $k>1$. So it can not be construct using ring operations. 

In a field like $ℤ_5$ such functions exist, for example $3⋅x + x^3+x^4$. Without loss of generality we can assume the outputs are $0$ and $1$, as this adds only a linear transform. Similarly we can assume $a=0$, so the function has a factor $\prod_{x∈[0,b)]}(X - x)$ and is of degree at least $b$. This takes at least $\log_2 b$ multiply operations to construct.

##### Binary

Given shares $s_0…s_{n-1}$ in $ℤ_{2^k}$, we can locally convert these to values in $ℤ_{2^m}×ℤ_{2^n}$ by splitting the bits. The lower part are already valid shares as is. The upper part is close, but misses a carry value $c∈[0,n-1)$, or $\lfloor \log_2 n \rfloor$ bits. It suffices to evaluate $c$ in $ℤ_{2^m}$, so we need only $b = \min(m, \lfloor \log_2 n \rfloor)$ bits.

The carry bits do not contain any revealing information [citation needed].

We can compute this carry value by combining the shares in a larger ring $ℤ_{2^{n + b}}$. But this would also reveal the lower bits.

[PSSY20]

Can we do lifting? Given a secret in $ℤ_{2^{n}}$ create a secret in $ℤ_{2^{n + b}}$? If such a process is efficient we can then use it to subtract the sensitive lower bits.

In $ℤ_2$ we can construct binary operations $∧, ⊕$ as $⋅,+$. This allows us to algebraically express the two-value carry function $\mathrm{carry}_2(a, b) = a \cdot b$ and the three value carry function 

$$
\begin{aligned}
\mathrm{carry}_2(a, b) &= a ⋅ b \\
\mathrm{carry}_3(a, b, c) &= a ⋅ b
\end{aligned}
$$

Question: Is there a two-value carry function for larger domains? $ℤ_{2^{n}}×ℤ_{2^{n}} → ℤ_{2}$. E.g. we are looking for an $f ∈ ℤ_4[X,Y]$ s.t.

$$
f(X, Y) = \left⌊ \frac{X + Y}{4} \right⌋
$$

Where the RHS is evaluated over $ℕ$.

$$
\begin{array}{c|cccc}
&   0 & 1 & 2 & 3 \\ \hline
0 & 0 & 0 & 0 & 0 \\
1 & 0 & 0 & 0 & 1 \\
2 & 0 & 0 & 1 & 1 \\
3 & 0 & 1 & 1 & 1 \\
\end{array}
$$

This can not exists, as $f(2,Y)$ would be the sign function polynomial I claimed was impossible before.


Question: If we re-interpret $ℤ_4$ as Galois Field $𝔽_4$, i.e. as $𝔽_2[X] / (X)$ or $𝔽_2[X] / (1 + X)$, can we then create this function?


$$
f(a, b) = a^2⋅b^2 + 3⋅a^2⋅b + 3⋅a⋅b^2 + 2⋅a⋅b + a + b
$$

$$
(a ⋅ b + 2) ⋅ (a ⋅ b + 3 ⋅ (a + b))
$$

This can be evaluated in two rounds.

$$
\begin{array}{c|cccc}
f & 00 & 01 & 10 & 11 \\ \hline
00 & 0 & 0 & 0 & 0 \\
01 & 0 & 0 & 0 & 1 \\
10 & 0 & 0 & 1 & 1 \\
11 & 0 & 1 & 1 & 1 \\
\end{array}
$$

##### Operations

* Secret shared as arithmetic sum.
    * Addition / subtraction.
    * Scalar multiplication.
    * Reduction to subgroup.
    * Replicated
        * Multiplication
* 

---

For each iriscode $\vec b_i$ in the database $\mathtt{DB}$ we convert it to a masked bitvector embedded in the ring $ℤ_{2^{16}}$. We then create secret shares such that each party $j$ has a share $\vec b_{ij}$.

Given a query iriscode $\vec a$, also encoded as a masked bitvector embedded in $ℤ_{2^{16}}$, but not converted to secret shares. Each party $j$ computes rotations of the query $r∈[-15,15]$ and computes the following with each database share $\vec b_{ij}$:

$$
d_{ijr} = \vsum(\mathtt{rot}(\vec a, r) ⊙ \vec b_{ij})
$$

Each party produces $31 ⋅ |\mathtt{DB}|$ values this way. These values are aggregated in the coordinator, which can decrypt the results

$$
d_{ir} = \sum_j d_{ijr} = \vsum(\mathtt{rot}(\vec a, r) ⊙ \vec b_{i})
$$

The coordinator has a database of only the masks $\vec b_{i\ \mathrm{m}}$ as regular bitvectors. This allows it to compute

$$
m_{ir} = \popcount(\mathtt{rot}(\vec a_{\mathrm{m}}, r) ∧ \vec b_{i\,\mathrm{m}})
$$

With both these pieces of information, the coordinator can compute the $\fhd$ between the query $\vec a$ and database entry $\vec b_i$.

$$
\mathtt{dist}(\vec a, \vec b_i) =
\min_{r∈[-15,15]}\ \left( \frac12 - \frac
{d_{ir}}
{2⋅m_{ir}} \right)
$$

To see that this work we can substitute back all the definitions from above:

$$
\begin{aligned}
\mathtt{dist}(\vec a, \vec b_i) &=
\min_{r∈[-15,15]}\ \left( \frac12 - \frac
{d_{ir}}
{2⋅m_{ir}} \right) \\
\min_{r∈[-15,15]}\ \fhd(\mathtt{rot}(\vec a, r), \vec b_i) &=
\min_{r∈[-15,15]}\ \left( \frac12 - \frac
{\sum_j d_{ijr}}
{2⋅\popcount(\vec a_{\mathrm{m}} ∧ \vec b_{i\,\mathrm{m}})} \right) \\
\fhd(\mathtt{rot}(\vec a, r), \vec b_i) &=
\frac12 - \frac
{\vsum(\mathtt{rot}(\vec a, r) ⊙ \vec b_{i})}
{2⋅\vsum\left((\mathtt{rot}(\vec a, r) ⊙ \vec b_{i})^{⊙2}\right)} \\
\end{aligned}
$$

## References

* [MRZ15] Payman Mohassel, Mike Rosulek, and Ye Zhang (2015). Fast and Secure Three-party Computation: The Garbled Circuit Approach.
* [AFL+16] Toshinori Araki, Jun Furukawa, Yehuda Lindell, Ariel Nof, and Kazuma Ohara (2016) High-Throughput Semi-Honest Secure Three-Party Computation with an Honest Majority.
* [MZ17] Payman Mohassel and Yupeng Zhang (2017). SecureML: A System for Scalable Privacy-Preserving Machine Learning.
* [MR18] Payman Mohassel and Peter Rindal (2018). ABY3: A Mixed Protocol Framework for Machine Learning.
* [CRS19] Harsh Chaudhari, Rahul Rachuri, and Ajith Suresh (2019). Trident: Efficient 4PC Framework for Privacy Preserving Machine Learning.
* [BCPS19] Megha Byali, Harsh Chaudhari, Arpita Patra, and Ajith Suresh (2019). FLASH: Fast and Robust Framework for Privacy-preserving Machine Learning.
* [PS20] Arpita Patra and Ajith Suresh (2020). BLAZE: Blazing Fast Privacy-Preserving Machine Learning.
* [PSSY20] Arpita Patra, Thomas Schneider, Ajith Suresh, and Hossein Yalame (2020). ABY2.0: Improved Mixed-Protocol Secure Two-Party Computation.

[MRZ15]: https://eprint.iacr.org/2015/931.pdf
[AFL+16]: https://eprint.iacr.org/2016/768.pdf
[MZ17]: https://eprint.iacr.org/2017/396.pdf
[MR18]: https://eprint.iacr.org/2018/403.pdf
[CRS19]: https://eprint.iacr.org/2019/1315.pdf
[BCPS19]: https://eprint.iacr.org/2019/1365.pdf
[PS20]: https://eprint.iacr.org/2020/042.pdf
[PSSY20]: https://eprint.iacr.org/2020/1225.pdf


