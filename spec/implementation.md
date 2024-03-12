# Implementation

Following the [protocol specification](./specification.md) the following needs to be computed:

## Implementing $𝔽_{2^{16} - 17}$

## Operating cost

Considering two machines:

* [m7gd.metal] Graviton 3, 64 cores, 256GB mem, 30 Gb/s,  $2025.31/month (spot), $918.70/month (3y reserved).
* [p5.48xlarge] AMD EPYC 7R13 (192 cores), 2TB mem,  8x Nvidia H100 , 3200 Gb/s. $76,797/month (spot).  $33,709/month (3y reserved).

[m7g.16xlarge]: https://instances.vantage.sh/aws/ec2/m7gd.metal?region=eu-north-1&cost_duration=monthly
[p5.48xlarge]: https://instances.vantage.sh/aws/ec2/p5.48xlarge?region=eu-north-1&cost_duration=monthly


### Benchmarks

### Communication

## Future Optimizations

### Secret sharing scheme

We can further explore the design space of secret sharing schemes and look into Packed Secret Sharing (PSS) to allow more computation with the same number of shares.

**Note.** These changes are not backwards compatible and requires converting the existing shares. Migrating the database is easy enough (though might be computationally/bandwidth costly), but the generation of shares on Orbs/user devices also needs to be upgraded. And there is a transition period during which old-scheme queries need to be on-the-fly converted before querying the new database.

### MSB extraction

We can explore different methods to reveal the comparison result.

Given double randomness $[r]_{\mathsf{MPC}}, [r]_{\mathsf{FHE}}$ we can efficiently convert between the two and do part of the computation, such as MSB extraction in FHE.

### Matmul implementation

The critical operation of matrix multiplication (over a ring) can be optimized further. Any such improvement is backwards compatible.

*   https://medium.com/@zhaodongyu/optimize-sgemm-on-risc-v-platform-b0098630b444
    Interesting point on prepacking.
*   https://github.com/timocafe/strassen
    clever method to combine CPU + GPU.
*   https://www.cise.ufl.edu/~sahni/papers/strassen.pdf
    Winograd's variant of Strassen multiplication.
    Also details GPU implementation.
*   https://dl.acm.org/doi/10.1145/3087556.3087579
    Further improvement on Strassen.
*   https://www-auth.cs.wisc.edu/lists/theory-reading/2009-December/pdfmN6UVeUiJ3.pdf
    Coppersmith-Winograd.

**Note.** Fast matrix multiplication algorithms are mostly avoided because of numerically stability reasons, which does not matter when working in rings which are always exact.
