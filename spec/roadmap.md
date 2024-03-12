# Iris code SMPC

## Background

### Iris code derivation

What are iris codes?

### High level

Orb + User -> Iriscode -> Uniqueness service -> On-chain merkle tree.

### Iris codes

Image segmentation -> unrolled -> convolution -> binarization.

### Iris code uniqueness check

* Fractional hamming distance.
* Rotations.

## Uniqueness service

* v0: Centralized database. Access controls.
    * 
* v1: Secret database at rest. (cleartext queries, masks, distances). 2 parties.
    * {-1,0,1} representation. Dot-product in LSSSs.
    * X Graviton 3.
* v2: Secret data in flight. (secret queries, masks and distances. Cleartext matches.) Semi-honest. TEE for integrity. 2 or 3 party.
    * Dot 
* v3: Upgrading from semi-honest to adverserial?

* Parallel workstreams:
    * Orb authenticating camera, signs raw image commitment.
    * Self-custodial iris-codes, zk-proof of correct derivation.
    * Shares encrypted for parties. (zk-proof attests to correct *encrypted* shares).
    * Parties sign 
    * L1 zk-proof attests to 

## v1 design


## Questions

* Semi-honest parties stick to protocol. Guarantees that they can not learn the secret. Adverserial party may manipulate outcome.
  * What can an adverserial party *learn* by manipulating the outcome?
* Efficient



v0:
For the old uniqueness check we're using c7g.8xlarge instance types. Currently we're running 4 partitions (to handle the 3.44M codes in the db) and we have an additional partition in stand by to handle codes in the range [4M, 5M) once we get there. Each partition has 3 replicas, so it would be 12 c7g.8xlarge for the current amount of codes and they'll become 15 when we'll get to 4M codes. For legacy uniqueness check we don't distinguish between left and right, each node processes both of them.

* 15 x [c7g.8xlarge] (Graviton 3, 32 cores 64 GB mem). Total 480 cores, 960 GB.

[c7g.8xlarge]: https://instances.vantage.sh/aws/ec2/c7g.8xlarge

v1 of MPC I guess? Currently we run 6 m7g.16xlarge for the coordinators, 6 m7g.16xlarge for the participants in organization 1 and 6 for the participants in the other org. Total 18 m7g.16xlarge, of which 9 for left and 9 for right. Although the coordinators could be moved to a smaller machine, we'll need to do that

* 18 x [m7g.16xlarge] (Graviton 3, 64 cores, 256GB mem, 30 Gb/s). Total 1152 cores, 4608 GB, 540 Gb/s.

[m7g.16xlarge]: https://instances.vantage.sh/aws/ec2/m7g.16xlarge
