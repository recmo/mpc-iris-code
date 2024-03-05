# Specification

$$
\gdef\delim#1#2#3{\mathopen{}\mathclose{\left#1 #2 \right#3}}
\gdef\p#1{\delim({#1})}
\gdef\ps#1{\delim\{{#1}\}}
\gdef\box#1{\delim[{#1}]}
\gdef\vec#1{\mathbf{#1}}
\gdef\mat#1{\mathrm{#1}}
\gdef\setn#1{\mathcal{#1}}
\gdef\sss#1{\mathcal{#1}}
\gdef\T{\mathsf{T}}
\gdef\F{\mathsf{F}}
\gdef\U{\mathsf{U}}
\gdef\popcount{\mathtt{popcount}}
\gdef\count{\mathtt{count}}
\gdef\fhd{\mathtt{fhd}}
\gdef\vsum{\mathtt{sum}}
$$

## MPC

Denote with $[–]_{\sss S}$ an encoding in a Linear Secret Sharing Scheme (LSSS) $\sss S$ over some ring $𝕂$ such that for $a ∈ 𝕂$ the encoded secret is $[a]_{\sss S}$. Similarly for $\vec a ∈ 𝕂^n$ let $[\vec a]_{\sss S}$ represent the $n$ shared secrets to encode $\vec a$. The linearity (really affinity) allows us to compute $[\vec b]_{\sss S} = \mat A ⋅ [\vec a]_{\sss S} + \vec c$ locally without communication for any $\mat A ∈ 𝕂^{m×n}, \vec c ∈𝕂^m$. Furthermore there are protocols (potentially with communication) to compute

 *  The product of two values $[a]_{\sss S}, [b]_{\sss S}$ in some output scheme $\sss O$, denoted as
    $$[a ⋅ b]_{\sss O} = [a]_{\sss S}⋅[b]_{\sss S}$$
 *  The inner product of two values $[\vec a]_{\sss S}, [\vec b]_{\sss S}$:
    $$[\vec a ⋅ \vec b]_{\sss O} = [\vec a]_{\sss S}⋅[\vec b]_{\sss S}$$
*   The conversion between two schemes $[a]_{\sss A}$ and $[a]_{\sss B}$, which may or may not have the same ring $𝕂$.

## Iris codes

First we need some definitions and theory on masked bitvectors and their representation in a ring $𝕂$. 

### Binary operations in rings

Given a bitvector $\vec b ∈ \{\F,\T\}^n$ of length $n$, we take the usual binary operations of *not* $¬$, *and* $∧$, *or* $∨$, *xor* $⊕$, and also $\popcount$. We can embed this in a ring $𝕂$ by representing $\F,\T$ as $0,1$ respectively and using the following operations on vectors $\vec b ∈ 𝕂^n$:

$$
\begin{aligned}
¬ \vec a &= 1 - \vec a \\
\vec a ∧ \vec b &= \vec a ⊙ \vec b \\
\vec a ∨ \vec b &= \vec a + \vec b - \vec a ⊙ \vec b \\
\vec a ⊕ \vec b &= \vec a + \vec b - 2 ⋅ \vec a ⊙ \vec b \\
\popcount(\vec a) &= \vsum(\vec a) \\
\end{aligned}
$$

Where $⊙$ is the element wise product and $\vsum$ adds the vector elements. Note that $\vsum(\vec a ⊙ \vec b)$ is the vector dot product $\vec a ⋅ \vec b$. Note also that the $\popcount$ will be computed in the ring, so a sufficiently large ring is required.

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
{2⋅\vsum\left((\vec a ⊙ \vec b)^{⊙2}\right)} \\
\end{aligned}
$$

Note that $(\vec a ⊙ \vec b)^{⊙2} = \vec a^{⊙2} ⊙ \vec b^{⊙2}$ and thus depends only on the masks of $\vec a$ and $\vec b$. Given the masks $\vec a_m$ and $\vec b_m$ it can be computed in binary as

$$
\vsum\left((\vec a ⊙ \vec b)^{⊙2}\right) = \popcount(\vec a_m ∧ \vec b_m)
$$

### Iris codes

The iris code is a $12\ 800$-bit masked bitvector. The $12\ 800$ masked bitvectors can be interpreted as $64 × 200$ bit matrices. We can then define a rotation as a permutation on the columns:

$$
\mathtt{rot}(\vec b, n)[i,j] = \vec b[i,(j+n)\ \mathrm{mod}\ 200]
$$

The *distance* between two iriscodes $\mathtt a$ and $\mathtt b$ is defined as the minimum distance over rotations from $-15$ to $15$:

$$
\mathtt{dist}(\vec a, \vec b) = \min_{r∈[-15,15]}\ \fhd(\mathtt{rot}(\vec a, r), \vec b)
$$

Two iris codes are a match if their distance is less than some threshold

$$
\mathtt{dist}(\vec a, \vec b) < \mathrm{threshold}
$$

The main query we are interested in computing (see [matching modes](./matching-modes.md)) is: given an iris code $\vec q$, a large set of iris codes $\mathrm{DB}$, a subset of indices $\setn I$, and a threshold $t$ return

$$
\set{i ∈ \setn I \mid \mathtt{dist}(\vec q, \mathrm{DB}[i]) < t }
$$

## Iriscode SMPC v2

[Requirements](./requirements.md): iris code bits and mask secret (both queries and database), distances secret. Threshold plaintext. It is acceptable to leak individual match bits for rotations.

Take $𝕂$ to be a ring large enough to represent the popcount (e.g. $ℤ_{2^{16}}$ or $𝔽_{2^{16} - 17}$). Iris codes are encoded as $12,800$ dimensional masked bitvectors over this ring.

We have a database $\mat D ∈ 𝕂^{N×12,800}$ encoded as $[\mat D]$. Given a query $\vec q ∈ 𝕂^{12,800}$ as $[\vec q]$, a threshold $t$ and an index set $\setn I$, we want to return the result

$$
\set{i ∈ \setn I \mid \mathtt{dist}([\vec q], [\mat D_i]) < t }
$$

### Rotations to queries

Observe that a $\mathtt{dist}$ threshold checks is the logical 'or' of 31 rotated fractional hamming threshold checks, i.e. the following are equivalent:

$$
\mathtt{dist}(\vec a, \vec b) < t
$$

$$
\p{\min_{r∈[-15,15]}\ \fhd(\mathtt{rot}(\vec a, r), \vec b)} < t
$$

$$
\bigvee_{r∈[-15,15]}\ \p{\fhd(\mathtt{rot}(\vec a, r), \vec b) < t}
$$

Since a rotation is a permutation it can be computed locally (as permutations can be represented by matrices). So for a given query $[\vec q]$ we pre-compute $31$ queries

$$
[\vec q_r] = \mathtt{rot}([\vec q], r)
$$

This allows us to compute

$$
⋃_{r∈[-15,15]}
\set{i ∈ \setn I \mid \fhd([\vec q_r], [\mat D_i]) < t }
$$

We do this by computing and revealing individual comparison bits $[\fhd(\vec q_r, \mat D_i) < t]$ and aggregating indices in cleartext.

### Fractional hamming threshold

Given $[\vec q]$, $[\vec d]$, and $t$ we want to compute and reveal $[\fhd(\vec q, \vec d) < t]$. Expanding definitions and simplifying we get

$$
\begin{aligned}
\fhd(\vec q, \vec d) & < t \\
\frac{1}{2} -\frac
{ \vsum\p{\vec q ⊙ \vec d}}
{2⋅\vsum\p{(\vec q ⊙ \vec d)^{⊙2}}}
&< t \\
\frac
{ \vsum\p{\vec q ⊙ \vec d}}
{\vsum\p{(\vec q ⊙ \vec d)^{⊙2}}}
&> 1 - 2⋅t 
\end{aligned}
$$

We now need to get it from the rational to the integer domain:

$$
\vsum\p{\vec q ⊙ \vec d} > \p{1 - 2⋅t}⋅\vsum\p{(\vec q ⊙ \vec d)^{⊙2}}
$$

Approximate $\p{1 - 2⋅t}$ by some fraction $\frac ab$, then this becomes an integer sign check

$$
b ⋅ \vsum\p{\vec q ⊙ \vec d} - a⋅\vsum\p{(\vec q ⊙ \vec d)^{⊙2}} > 0
$$

$$
b ⋅ \p{[\vec q] ⋅ [\vec d]} - a⋅[\vsum\p{(\vec q ⊙ \vec d)^{⊙2}} > 0
$$

**Q.** Do we want the quartic equation or a separate encrypted mask? Assume separate for now.

We thus need to compute the sign of the expression

$$
b ⋅ \p{[\vec q] ⋅ [\vec d]} - a ⋅ \p{[\vec q_{\mathrm m}] ⋅ [\vec d_{\mathrm m}]}
$$

To do this we first compute the two dot products $[\vec q] ⋅ [\vec d]$ and $[\vec q_{\mathrm m}] ⋅ [\vec d_{\mathrm m}]$ in a ring $|𝕂| ≥ 12,800$. Then we lift these results to a larger ring using double-randomness and compute the above integer. It should suffice to have $|𝕂| > 12,800⋅\p{a + b}$.

Finally we apply a most-significant-bit extraction protocol to obtain the result of the comparison.

## Cost analysis

* Two masked bit vectors per query and database entry in a ring $|𝕂| ≥ 12,800$.
* Two dot products per (rotated) query.
* Two conversions to larger ring.
    * Two double-randomness generations.
    * Two decoding operations.
* One MSB extraction.

Concrete numbers:

* ABY3 (3 party, small ring $ℤ_{2^{16}}$):
    * Database: 8-bytes per record.
    * Computation: $2⋅2$ times a $12,800$ sized `u16×u16→u16` dot product.
    * Communication: ?
* Shamir (3 party, field $𝔽_{2^{16}-17}$):
    * Database: 4-bytes per record.
    * Computation: $2$ times a $12,800$ sized `u16×u16→u32` dot product.
    * Communication: ?

## Appendix

**Q.** How accurate does the threshold need to be? The reference implementation uses `f64`, which has $53$ bits of precision. Add to this the ~14 bits we need for the popcounts and the above expression would need to be evaluated in $67$ bits!

**Q.** In Shamir the interpolation polynomial $P(X)$ has substantial additional structure in that $P(x_s) ∈ \set{-1,0,1}$, i.e. $P(x_s)$ is a root of $X^3-X$. Can this be used as a 'low-degree' constraint? The goal here is to find a way to locally compute the quartic term on the RHS.

**Q.** In Shamir, if we want to square a number we have more constraints on the output polynomial. Can we use this as a substitute for degree reduction?

---

**Idea.** An interesting observation due to Bryan is the near-simplification:

$$
\begin{aligned}
&\vsum([b ⋅ \vec a + 2 ⋅ a ⋅ \vec a_{\mathrm m}]_{\mathrm r} ⊙ [\vec b + \vec b_{\mathrm m}]_{\mathrm r}) \\
&=
\vsum([b ⋅ \vec a]_{\mathrm r} ⊙ [\vec b ]_{\mathrm r}) +\vsum([2 ⋅ a ⋅ \vec a_{\mathrm m}]_{\mathrm r}) ⊙ [\vec b_{\mathrm m}]_{\mathrm r}) \\
&+ \vsum([b ⋅ \vec a]_{\mathrm r} ⊙ [\vec b_{\mathrm m}]_{\mathrm r}) +\vsum([2 ⋅ a ⋅ \vec a_{\mathrm m}]_{\mathrm r}) ⊙ [\vec b]_{\mathrm r}) \\
\end{aligned}
$$

The cross-terms here are noisy, but if the iris code are centered in that their expected value is zero, then both these cross terms also have an expected value of zero. With this trick the performance is essentially the same as in v0.
