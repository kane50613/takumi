---
packages:
  takumi-core: patch
---

# Animations sample their keyframes as browsers do

- A property that a middle keyframe leaves out interpolates between the keyframes that declare it, instead of passing through the element's own value.
- An `animation-timing-function` declared in a keyframe eases the interval that keyframe starts.
- A repeating animation shows the start of its next iteration at a boundary, so `1s infinite` at 1000ms draws its first keyframe.
- A zero-duration animation applies only with `animation-fill-mode: forwards` or `both`.
- A keyframe declaration marked `!important` is invalid, so the animation leaves that property alone.
- An animation with `animation-play-state: paused` stays at its start whatever `timeMs` is. To hold a fixed frame, pick it with a negative `animation-delay`.
- `steps()` in a reversed animation lands on the step Chrome shows before it starts and after it ends.
- `animation-iteration-count: 0` with `forwards` fill shows the first keyframe instead of the last.
- Two keyframes at one offset with different timing functions each ease only the properties they declare.
- `@keyframes` names match case-sensitively, so `animation-name: Fade` no longer runs `@keyframes fade`.

```css
@keyframes grow {
  0% {
    width: 0;
    opacity: 0;
  }
  50% {
    opacity: 1;
  }
  100% {
    width: 100px;
    opacity: 0;
  }
}
/* At 25%, before: width eases toward the element's own width */
/* At 25%, after: width is 25px */
```
