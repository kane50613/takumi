// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { Color } from "./Color";

/**
 * Represents a gradient with color steps and an angle for directional gradients.
 */
export type Gradient = { 
/**
 * The color stops that make up the gradient
 */
stops: Array<Color>, 
/**
 * The angle in degrees for the gradient direction (0-360)
 */
angle: number, };
