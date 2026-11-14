---
packages:
  takumi-core: patch
---

# A font registered under the built-in Geist's name draws before it

A font registered as `Geist` used to lose to the built-in Geist at weights 300 to 800, so its Cyrillic and Latin Extended characters drew as boxes. A family now reaches its last-resort faces only when no other face has it, and font fallback still reaches them after every registered family.
