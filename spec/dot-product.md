# Dot products

$$
\gdef\delim#1#2#3{\mathopen{}\mathclose{\left#1 #2 \right#3}}
\gdef\p#1{\delim({#1})}
\gdef\ps#1{\delim\{{#1}\}}
\gdef\floor#1{\delim\lfloor{#1}\rfloor}
\gdef\vec#1{\mathbf{#1}}
\gdef\mat#1{\mathrm{#1}}
$$

Given a commutative 16-bit modular ring $𝕂$, likely $ℤ_{2^{16}}$ or $𝔽_{2^{16} - 17}$.

Given $n=12,800$ and $\vec q, \vec d ∈ 𝕂^n$, we want to compute the dot product $c ∈ 𝕂$:

$$
c = \vec q ⋅ \vec d = \sum_i q_i ⋅ d_i
$$

We want to compute this for $N > 3,000,000$ vectors $\vec d_i$, which can be represented as a matrix $\mat D ∈ 𝕂^{n×N}$. Similarly we want to compute this for a batch of $m$ vectors $\vec q_j$, represented as $\mat Q ∈ 𝕂^{m×n}$. Then the $\mat C ∈ 𝕂^{m×N}$ result can be computed as

$$
\mat C = \mat Q ⋅ \mat D
$$

Since $m ≪ N$ it makes sense to see $\mat C$ and $\mat D$ as block matrices with block sizes $m×b$, $n×b$

$$
\begin{bmatrix}
\mat C_0 \\ 
\mat C_1 \\ 
\mat C_2 \\
⋮ 
\end{bmatrix}
= \mat Q ⋅
\begin{bmatrix}
\mat D_0 \\ 
\mat D_1 \\ 
\mat D_2 \\
⋮ 
\end{bmatrix}
$$

Batch size: $1-10$ requests per second, $31×$ increase gives $31—310$ per sec, adding up to 10 second latency gives $m ∈ [310,3100]$.

Block size: No constraint, optimize for performance.

### Shamir case

We have three parties with one share each. To multiply two secrets the shares are multiplied. Multiplications are modulo $2^{16} - 17$, but we can accumulate in `u32` and delay the reduction.

### Replicated case

Each party has two shares of each secret, to compute a product they must compute 

$$
\p{a_0 + a_1}⋅\p{b_0 + b_1} - a_1⋅b_1
$$

We can preprocess the shares such that this becomes

$$
a_0⋅b_0 + a_1⋅b_1
$$

In batched matrix form, this becomes the sum of two matrix multiplications

$$
\mat C = \mat Q_0 ⋅\mat D_0 + \mat Q_1 ⋅ \mat D_1
$$

which itself are just larger matrix multiplications

$$
\mat C = 
\begin{bmatrix}
\mat Q_0 & \mat Q_1
\end{bmatrix}
⋅
\begin{bmatrix}
\mat D_0 \\ \mat D_1
\end{bmatrix}
$$

So we can equally treat it as if the code $n$ is now twice as long, $25,600$ elements.
