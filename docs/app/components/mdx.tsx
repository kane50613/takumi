import { Card, Cards } from "fumadocs-ui/components/card";
import { Step, Steps } from "fumadocs-ui/components/steps";
import { Tab, Tabs } from "fumadocs-ui/components/tabs";
import defaultMdxComponents from "fumadocs-ui/mdx";
import * as Twoslash from "fumadocs-twoslash/ui";
import {
  ArrowBigRight,
  BookOpen,
  FileCode2,
  Hand,
  Palette,
  Shovel,
  Terminal,
  ToyBrick,
  Type,
  Wrench,
  Zap,
} from "lucide-react";
import type { MDXComponents } from "mdx/types";
import { Accordion, Accordions } from "./accordion";
import { Mermaid } from "./mdx/mermaid";
import { LocalSpace, PaintPipeline, RenderPipeline, StepReplay, TextRuns } from "./mdx/paint";
import { TypeTable } from "./type-table";
import { Video } from "./video";

export default {
  ...defaultMdxComponents,
  ...Twoslash,
  Hand,
  BookOpen,
  FileCode2,
  Wrench,
  Shovel,
  ToyBrick,
  ArrowBigRight,
  Zap,
  Palette,
  Terminal,
  Type,
  Mermaid,
  PaintPipeline,
  RenderPipeline,
  StepReplay,
  LocalSpace,
  TextRuns,
  Step,
  Steps,
  Card,
  Cards,
  Tab,
  Tabs,
  Accordion,
  Accordions,
  TypeTable,
  Video,
} satisfies MDXComponents;
