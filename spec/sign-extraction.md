# Sign extraction

$$
\gdef\vec#1{\mathbf{#1}}
\gdef\mat#1{\mathrm{#1}}
$$

Given a secret value $[a]$ in $ℤ_{2^k}$ in a linear secret sharing scheme. We want to know if $a > 2^{k-1}$, i.e. the value of the most significant bit.

The secret is shared in 3 shares $a_0, a_1, a_2$ such that $a = a_1 + a_2 + a_3$ in $ℤ_{2^k}$.

The current best proposal is to convert each share to a secret in $ℤ_2^k$ and implement a carry-save adder followed by a ripple carry adder to compute the result.

**Re-randomization**. The three parties can create a share of $[z] = [0]$ that they can add to a secret to re-randomize it. Consider three pseudo-random number generators $\mathrm{prng}_i$, where each party has access to two of them (and they are kept synchronized). Then each party computes

$$
\begin{aligned}
z_0 &= \mathrm{prng}_0() - \mathrm{prng}_1() \\
z_1 &= \mathrm{prng}_1() - \mathrm{prng}_2() \\
z_2 &= \mathrm{prng}_2() - \mathrm{prng}_0() \\
\end{aligned}
$$

TODO: This creates individual shares and will requiring replication.  Party 0 receives z1 from party 1. But party 0 also knows prng_1, so it can now compute prng_2 and now all the values.

**Secret sharing.** If a party $i$ has a cleartext value $a$ they can share this as a secret $[a]$ by first creating a randomized $[0]$ and then party $i$ adds $a$ to their share. This creates a secret value $[a]$.

**Conversion.** Given a secret $[a]$ in $ℤ_{2^k}$. Each party $i$ can take their share, $a_i$, apply a number of arbitrary mappings $\vec f_{ij}: ℤ_{2^k} → ℤ_2^k$, and secret share it, to create secret values:

$$
\begin{aligned}
[\vec a_{0j}] &= [\vec f_{0j}(a_0)] \\
[\vec a_{1j}] &= [\vec f_{1j}(a_1)] \\
[\vec a_{2j}] &= [\vec f_{2j}(a_2)] \\
\end{aligned}
$$

If the mappings $f_{ij}$ are expressing the value as a twos-complement binary $\mathsf{bits}$, then an boolean adder circuit can be used to combine the shares:

$$
[\vec z] = \mathsf{adder}([\mathsf{bits}(a_0)], [\mathsf{bits}(a_0)], [\mathsf{bits}(a_2)])
$$

If we are starting from a replicated secret $[a]$ in $ℤ_{2^k}$. Each party can take their shares, $a_i, a_{i+1}$, apply anumber of arbitrary mappings $\vec f_{ij}: ℤ_{2^k}^2 → ℤ_2^k$, and secret share it, to create secret values:

$$
\begin{aligned}
[\vec a_{0j}] &= [\vec f_{0j}(a_0, a_1)] \\
[\vec a_{1j}] &= [\vec f_{1j}(a_1, a_2)] \\
[\vec a_{2j}] &= [\vec f_{2j}(a_2, a_0)] \\
\end{aligned}
$$

In particular we can pick $f_{00}(a_0, a_1) = \mathsf{bits}(a_0 + a_1)$ and $f_{10}(a_1, a_2) = \mathsf{bits}(a_2)$ then we need only a two argument adder:

$$
[\vec z] = \mathsf{adder}([\mathsf{bits}(a_0 + a_1)], [\mathsf{bits}(a_2)])
$$

---

Now $[\vec z]$ should hold the twos-complement binary of the original secret value $a$. This allows us to extract the most significant bit.

There are many ways to implement the adder circuit. One efficient way is to use a carry-save adder. Here we first we add each triplet of bits together.

$$
\begin{aligned}
[\vec s] &= [\vec a_0] ⊕ [\vec a_1] ⊕ [\vec a_2] \\
[\vec c] &= [\vec a_0] ∧ [\vec a_1] ⊕ [\vec a_2] ∧ ([\vec a_0] ⊕ [\vec a_1]) \\
\end{aligned}
$$

Note that the carry expression is symmetric in the arguments, but the asymetric form saves and $\mathsf{and}$ operation:

$$
a ∧ b ⊕ c ∧ (a ⊕ b) = a ∧ b ⊕ b ∧ c ⊕ c ∧ a
$$

Then we need to compute $[\vec a] = 2⋅[\vec c] + [\vec s]$, we can do this through a ripple-carry adder:

$$
\begin{aligned}
[z_0] &= [s_0] \\
[z_1] &= [s_1] ⊕ [c_0]          &  [t_1] &= [s_1] ∧ [c_0] \\
[z_2] &= [s_2] ⊕ [c_1] ⊕ [t_1]  &  [t_2] &= [s_2] ∧ [c_1] ⊕ [t_1] ∧ ([s_2] ⊕ [c_1]) \\
[z_3] &= [s_3] ⊕ [c_2] ⊕ [t_2]  &  [t_3] &= [s_3] ∧ [c_2] ⊕ [t_2] ∧ ([s_3] ⊕ [c_2]) \\
&\ \ ⋮ & &\ \ ⋮ \\ 
[z_{k-2}] &= [s_{k-2}] ⊕ [c_{k-3}] ⊕ [t_{k-3}]  &  [t_{k-2}] &= [s_{k-2}] ∧ [c_{k-3}] ⊕ [t_{k-3}] ∧ ([s_{k-2}] ⊕ [c_{k-3}]) \\
[z_{k-1}] &= [s_{k-1}] ⊕ [c_{k-2}] ⊕ [t_{k-2}]  \\
\end{aligned}
$$

Note that we compute the addition in $ℤ_{2^k}$ so we ignore the overflow. Consequently we do not need to compute $[c_{k-1}]$ in the previous step.

Complexity wise, the carry-save step requires $2⋅(k-1) ⋅ \mathsf{and}$ operation, which can all be done in parallel. The ripple-carry phase requires $(k-2)⋅ \mathsf{and}$ operations that can all be done in parallel and $(k-3)⋅ \mathsf{and}$ operations that have a data dependency.

**MSB Extraction** Note that for MSB extraction we are only interested in computing $[z_{k-1}]$, so working backwards, the computation rounds are

$$
\begin{aligned}
[s_i] &= [a_{0,i}] ⊕ [a_{1,i}] ⊕ [a_{2,i}] \\
[c_i] &= [a_{0,i}] ∧ [a_{1,i}] ⊕ [a_{2,i}] ∧ ([a_{0,i}] ⊕ [a_{1,i}]) \\
[z_{k-1}] &= [s_{0,k-1}] ⊕ [c_{k-2}] ⊕ [t_{k-2}] \\
\end{aligned}
$$

$$
\begin{aligned}
\begin{bmatrix}
\end{bmatrix}
[s_i] &= [a_{0,i}] ⊕ [a_{1,i}] ⊕ [a_{2,i}] \\
[c_i] &= [a_{0,i}] ∧ [a_{1,i}] ⊕ [a_{2,i}] ∧ ([a_{0,i}] ⊕ [a_{1,i}]) \\
[z_{k-1}] &= [s_{0,k-1}] ⊕ [c_{k-2}] ⊕ [t_{k-2}] \\
\end{aligned}
$$

The full expression is computing a polynomial function in $𝔽_2[X_0,…X_{3⋅k -1}]$.

## Affine transforms and bi-linear algorithms

$$
a ∧ b ⊕ c ∧ (a ⊕ b) = a ∧ b ⊕ b ∧ c ⊕ c ∧ a
$$

We can represent this as a bilinear algorithm:

$$
\begin{aligned}
\begin{bmatrix}
1 & 1 & 1
\end{bmatrix} ⋅ \left(
\begin{bmatrix}
1 & 0 & 0 \\
0 & 1 & 0 \\
0 & 0 & 1 \\
\end{bmatrix} ⋅
\begin{bmatrix}
a \\ b \\ c
\end{bmatrix}
⊙
\begin{bmatrix}
0 & 1 & 0 \\
0 & 0 & 1 \\
1 & 0 & 0 \\
\end{bmatrix} ⋅
\begin{bmatrix}
a \\ b \\ c
\end{bmatrix}
\right)
\end{aligned}
$$


$$
\begin{aligned}
\begin{bmatrix}
1 & 1
\end{bmatrix} ⋅ \left(
\begin{bmatrix}
1 & 0 & 0 \\
0 & 0 & 1 \\
\end{bmatrix} ⋅
\begin{bmatrix}
a \\ b \\ c
\end{bmatrix}
⊙
\begin{bmatrix}
0 & 1 & 0 \\
1 & 1 & 0 \\
\end{bmatrix} ⋅
\begin{bmatrix}
a \\ b \\ c
\end{bmatrix}
\right)
\end{aligned}
$$

Each round of communication basically computes such a bilinear algorithm.

Note that the communication doesn't stem from the multiplication itself, but from the re-sharing required to obtain two shares per party.


In fact the slightly more generic version is affine transformations. I.e. we can compute locally

$$
\vec x' = \mat A ⋅ \vec x + \vec b
$$

Bilinear algorithms

$$
\vec z = \mat C ⋅ \left(\left(\mat B ⋅ \vec x \right)⊙ \left(\mat C ⋅ \vec y \right)\right)
$$


Let's see if affine bilinear algorithms add anything.

$$
\begin{aligned}
\vec z &= \mat C ⋅ \left(\left(\mat A ⋅ \vec x + \vec a\right)⊙ \left(\mat B ⋅ \vec y + \vec b\right)\right) \\
&= \mat C ⋅ \left(
    \left(\mat A ⋅ \vec x \right) ⊙ \left(\mat B ⋅ \vec y \right) +
    \left(\mat A ⋅ \vec x \right) ⊙ \vec b +
    \vec a ⊙ \left(\mat B ⋅ \vec y \right)
\right) +
\mat C ⋅ \left( \vec a ⊙ \vec b \right) \\
&= \mat C ⋅ \left(
    \left(\mat A ⋅ \vec x \right) ⊙ \left(\mat B ⋅ \vec y \right)
\right) +
\mat C ⋅ \mat A ⋅ \vec x ⋅ \mathrm{diag}(\vec b) +
\mat C ⋅ \mathrm{diag}(\vec a) · \mat B ⋅ \vec y +
\mat C ⋅ \left( \vec a ⊙ \vec b \right) \\
&= \mat C ⋅ \left(
    \left(\mat A ⋅ \vec x \right) ⊙ \left(\mat B ⋅ \vec y \right)
\right) +
\mat A' ⋅ \vec x  +
\mat B' ⋅ \vec y +
\vec c \\
\end{aligned}
$$

So what a single round of communication can compute is exactly the sum of a bilinear algorithm and an affine transform:

$$
\begin{aligned}
\vec s' &=
\mat C ⋅ \left(
    \left(\mat A ⋅ \vec s \right) ⊙ \left(\mat B ⋅ \vec s \right)
\right) +
\mat D ⋅ \vec s  +
\vec c \\
\end{aligned}
$$

## Re-interpreting the MSB extractor

$$
\begin{aligned}
\begin{bmatrix}
\vec s
\end{bmatrix}
&=
\begin{bmatrix}
\mat I & \mat I & \mat I 
\end{bmatrix}
⋅
\begin{bmatrix}
\vec a_0 \\ \vec a_1 \\ \vec a_2
\end{bmatrix}
+ \vec 0
\end{aligned}
$$

$$
\begin{aligned}
\begin{bmatrix}
\vec c
\end{bmatrix}
&=
\begin{bmatrix}
1 & 1 & 1
\end{bmatrix} ⋅ \left(
\begin{bmatrix}
1 & 0 & 0 \\
0 & 1 & 0 \\
0 & 0 & 1 \\
\end{bmatrix} ⋅
\begin{bmatrix}
\vec a_0 \\ \vec a_1 \\ \vec a_2
\end{bmatrix}
⊙
\begin{bmatrix}
0 & 1 & 0 \\
0 & 0 & 1 \\
1 & 0 & 0 \\
\end{bmatrix} ⋅
\begin{bmatrix}
\vec a_0 \\ \vec a_1 \\ \vec a_2
\end{bmatrix}
\right)
\end{aligned}
$$

**Question.** When converting from arithmetic to binary, we can locally compute, with resharing:

$$
[a_0], [a_1], [a_2], [a_0+a_1], [a_1+a_2], [a_2+a_0]
$$

---
## Extracting the sign bit

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

