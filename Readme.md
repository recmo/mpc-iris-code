# MPC Iris Code

**DO NOT USE IN PROD**

Experiments to see if iris codes can be matched in MPC with acceptable privacy and performance.

## Specification

For present purposes an iriscode consists of $\mathtt{data}$ bits and $\mathtt{mask}$, two $12\ 800$-bit bitvectors. When a mask bit is set the corresponding data bit should be ignored.

### Fractional hamming distance

A *fraction hamming distance* between two such iriscodes $\mathtt a$ and $\mathtt b$ is computed as

$$
\begin{aligned}
d &= \mathtt{a.data} ⊕ \mathtt{b.data} \\
\overline{m} &= \neg(\mathtt{a.mask} \vee \mathtt{b.mask}) \\
\mathrm{fhd}(\mathtt{a}, \mathtt{b}) &= \frac
{\mathtt{popcount}(d ∧ \overline{m})}
{\mathtt{popcount}(\overline{m})}
\end{aligned}
$$

where $d$ is a vector of data bits that are different, $\overline{m}$ is a vector of bits that are unmasked in $\mathtt a$ and $\mathtt b$ and $\mathtt{fhd}$ is the *fractional hamming distance*.

### Rotations

The $12\ 800$ bitvectors can be interpreted as $64 × 200$ bit matrices. We can then define a rotation as a permutation on the columns:

$$
\mathrm{rot}(\mathtt b, n)[i,j] = \mathtt b[i,(j+n)\ \mathrm{mod}\ 200]
$$

When applied to an iriscode this applies to $\mathtt{data}$ and $\mathtt{mask}$ equaly.

### Distance

The *distance* between two iriscodes $\mathtt a$ and $\mathtt b$ is defined as the minimum distance over rotations from $-15$ to $15$:

$$
\mathrm{dist}(\mathtt a, \mathtt b) = \min_{r∈[-15,15]}\ \mathrm{fhd}(\mathrm{rot}(\mathtt a, r), \mathtt b)
$$

### Uniqueness

To verify uniqueness we require that an iriscode $\mathtt a$ is a certain minimum distance from all previous iriscodes:

$$
\mathop{\Large ∀}\limits_{\mathtt b ∈ \mathtt{DB}}\ 
\mathrm{dist}(\mathtt a, \mathtt b) > \mathrm{threshold}
$$

where $\mathtt{DB}$ is the set of already registered iriscodes (currently 3 million entries).

When there is a match, we are also interested in finding the location of the best match. Both can be addressed by implementing a method that returns the index of the minimum distance entry.

## MPC specific notes

### Minimal proposal

Keep the query $\mathtt{a}$ and the $\mathtt{mask}$ s cleartext, but $\mathtt{DB}$ entries ciphertext.

In cleartext compute a rotation of $\mathtt{a}$ and

$$
\begin{aligned}
\overline{m} &= \neg (\mathtt{a.mask} ∨ \mathtt{b.mask}) \\
M &= \mathrm{popcount}(\overline{m}) \\
\end{aligned}
$$

### Lifting to $ℤ_{2^k}$

Given bitvector $\mathbf a$ and $\mathbf b$. Observe that if the bits are lifted to a large enough group $ℤ_{2^k}$ we can compute a popcount as a dot product

$$
\mathtt{popcount}(\mathbf a ∧ \mathbf b) = \sum_i a_i ⋅ b_i
$$

For this to work we need $2^k$ larger than the number of bits in the vector. We can also lift the $⊕$ operation to one in $ℤ_{2^k}$:

$$
\begin{aligned}
(\mathbf a ⊕ \mathbf b)_i
&= a_i ⋅ (1 - b_i) + (1 - a_i)⋅b_i \\
&= a_i  + b_i - 2 ⋅ a_i⋅ b_i
\end{aligned}
$$

Together this results in

$$
\begin{aligned}
\mathtt{popcount}((\mathbf a ⊕ \mathbf b) ∧ \mathbf c)
&= \sum_i (a_i  + b_i - 2 ⋅ a_i⋅ b_i) ⋅ c_i \\
&= \sum_i a_i⋅c_i  + \sum_i b_i⋅c_i - 2 ⋅ \sum_i a_i⋅ b_i ⋅ c_i \\
&= \sum_i a_i⋅c_i  + \sum_i b_i⋅c_i - 2 ⋅ \sum_i d_i⋅ b_i ⋅ c_i \\
\end{aligned}
$$

Note that $\mathbf a$ and $\mathbf c$ are cleartext, so we can compute $\mathbf d = \mathbf a ∧ \mathbf c$ and $e = \mathtt{popcount}(\mathbf d)$ in advance

$$
\begin{aligned}
&= e + \sum_i b_i⋅c_i - 2 ⋅ \sum_i d_i⋅ b_i \\
&= e + \sum_i b_i ⋅ (c_i - 2 ⋅ d_i) \\
\end{aligned}
$$

Computing $\mathbf f = \mathbf c - 2⋅\mathbf d$ in advance we get

$$
\begin{aligned}
&= e + \sum_i b_i⋅c_i - 2 ⋅ \sum_i d_i⋅ b_i \\
&= e + \sum_i b_i ⋅ f_i \\
\end{aligned}
$$

The values $f_i$ can have are $\{-1,0,1\}$ as can be seen by expanding into source bits

$$
f_i
= c_i - 2 ⋅ a_i ⋅ c_i 
= c_i ⋅ (1 - 2 ⋅ a_i) 
$$

This suggests an alternative method where we compute the set of negative and positive
indices, then we can just sum subsets of $\mathbf b$.

$$
\mathtt{popcount}((\mathbf a ⊕ \mathbf b) ∧ \mathbf c)
= e  - \sum_{i ∈ \mathbf c ∧ \mathbf a} b_i + \sum_{i ∈ \mathbf c ∧ ¬\mathbf a} b_i
$$


### Batching

---


**Idea.** Use Freivald's algorithm to verify the matrix multiplication in MPC setting.
