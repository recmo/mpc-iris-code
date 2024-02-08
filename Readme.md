# MPC Iris Code

**DO NOT USE IN PROD**

$$
\gdef\vec#1{\mathbf{#1}}
\gdef\T{\mathsf{T}}
\gdef\F{\mathsf{F}}
\gdef\U{\mathsf{U}}
\gdef\popcount{\mathtt{popcount}}
\gdef\count{\mathtt{count}}
\gdef\fhd{\mathtt{fhd}}
\gdef\vsum{\mathtt{sum}}
$$

Experiments to see if iris codes can be matched in MPC with acceptable privacy and performance.

## Install

Make sure to optimize for the correct target CPU to make use of SVE features. To do this set the compiler flag to optimize for the target CPU:

```sh
RUSTFLAGS="-Ctarget-cpu=native" cargo install --git https://github.com/recmo/mpc-iris-code
```

When cross-compiling as source checkout from a different environment to Graviton 3, set the cpu explicitly:

```sh
RUSTFLAGS="-Ctarget-cpu=neoverse-v1" cargo build --release --target aarch64-unknown-linux-gnu
```

To explore assembly output in [Godbolt], use the following compiler options:

[Godbolt]: https://rust.godbolt.org/

```
--edition=2021 --target aarch64-unknown-linux-gnu -C opt-level=3 -C lto=fat --C target-cpu=neoverse-v1
```

Some useful resources for low-level Apple Silicon and Graviton optimization:

* https://dougallj.github.io/applecpu/firestorm.html
* https://chipsandcheese.com/2022/05/29/graviton-3-first-impressions/

# Specification

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
&= \mathtt{fhw}(\vec a ⊕ \vec b) =\frac
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

## Iriscode SMPC v1

Objective: DB iriscode bits encrypted. Query, mask and distances unencrypted. 

For each iriscode $\vec b_i$ in the database $\mathtt{DB}$ we convert it to a masked bitvector embedded in the ring $ℤ_{2^{16}}$. We then create secret shares such that each party $j$ has a share $\vec b_{ij}$.

Given a query iriscode $\vec a$, also encoded as a masked bitvector embedded in $ℤ_{2^{16}}$, but not converted to secret shares. Each party $j$ computes rotations of the query $r∈[-15,15]$ and computes the following with each database share $\vec b_{ij}$:

$$
d_{ijr} = \vsum(\mathtt{rot}(\vec a, r) ⊙ \vec b_{ij})
$$

Each party produces $31 ⋅ |\mathtt{DB}|$ values this way. These values are aggregated in the coordinator, which can decrypt the results

$$
d_{ir} = \sum_j d_{ijr} = \vsum(\mathtt{rot}(\vec a, r) ⊙ \vec b_{i})
$$

The coordinator has a database of only the masks $\vec b_{i}^{⊙2}$ as regular bitvectors. This allows it to compute

$$
m_{ir} =
\vsum((\vec a ⊙ \vec b_i)^{⊙2}) = \vsum(\vec a^{⊙2} ⊙ \vec b_i^{⊙2}) = \popcount(\vec a_{\mathrm m} ∧ \vec b_{i \mathrm m})
$$

With both these pieces of information, the coordinator can compute the $\fhd$

$$
\fhd(\vec a, \vec b_i) = \frac
{\vsum(\vec a ⊙ \vec b)}
{\vsum((\vec a ⊙ \vec b)^{⊙2})} \\
$$

## References

