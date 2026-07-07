---
packages:
  npm:takumi-js:
    replay:
      - exit-prerelease(npm:takumi-js)
---

### Mark `ImageResponse.ready` rejection as handled

A failed render no longer crashes the process with an `unhandledRejection` when
the caller never awaits `ready`. The failure still reaches the stream and a
caller that does await `ready` still observes it.
