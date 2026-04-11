export interface CommandDescriptor<T = unknown> {
  _command: string;
  _arg: T;
}

export type Direction = "left" | "right" | "up" | "down";
export type NoArgCommand = () => CommandDescriptor<undefined>;
export type StringCommand = (arg: string) => CommandDescriptor<string>;
export type NumberCommand = (arg: number) => CommandDescriptor<number>;
export type DirectionCommand = (arg: Direction) => CommandDescriptor<Direction>;

export const spawn: StringCommand;
export const reload_config: NoArgCommand;
export const focus_next: NoArgCommand;
export const focus_prev: NoArgCommand;
export const focus_dir: DirectionCommand;
export const swap_dir: DirectionCommand;
export const resize_dir: DirectionCommand;
export const resize_tiled: DirectionCommand;
export const view_workspace: NumberCommand;
export const assign_workspace: NumberCommand;
export const toggle_workspace: NumberCommand;
export const toggle_floating: NoArgCommand;
export const toggle_fullscreen: NoArgCommand;
export const set_layout: StringCommand;
export const cycle_layout: NoArgCommand;
export const move: DirectionCommand;
export const resize: DirectionCommand;
export const kill_client: NoArgCommand;
