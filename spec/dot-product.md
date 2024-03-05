# Dot products

$$
\gdef\delim#1#2#3{\mathopen{}\mathclose{\left#1 #2 \right#3}}
\gdef\p#1{\delim({#1})}
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