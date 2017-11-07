import { float, int } from "../types";
import { EntityData, EntityInitData, EntityKind, EntityUpdateData, MapIndex } from "./EntityData";
import { Game } from "../Game";

export enum EntityState {
    // Waiting for `init` to be called
    PendingCreation,

    // `init` has been called
    Alive,

    // Destroying with an animation
    DestroyingAnimated,

    // Waiting to be removed
    PendingDestroy,

    // Removed
    Destroyed
}

export abstract class Entity extends PIXI.Container {
    // Basic properties
    public state: EntityState = EntityState.PendingCreation;
    public kind: EntityKind;
    public id: int = -1;
    public index: MapIndex = [0, 0];

    public get x(): int { return this.index[0]; }
    public set x(x: int) { this.index[0] = x; }
    public get y(): int { return this.index[1]; }
    public set y(y: int) { this.index[1] = y; }

    // Other parameters
    public autoPosition: boolean = false; // If the entity should automatically move to the given position

    public constructor() {
        super();
    }

    /// Called when inserted into the scene.
    public initEntity(data: EntityInitData) {
        // Set the ID and kind
        this.kind = data.kind;
        this.id = data.id;

        // Set the index
        this.x = data.index[0];
        this.y = data.index[1];
        this.position.set(Game.shared.indexToGlobalX(this.x), Game.shared.indexToGlobalY(this.y));
        // this.moveToArray(data.index);

        // Update the state
        this.state = EntityState.Alive;
    }

    /// Called when new data about the entity is received.
    public updateEntity(data: EntityUpdateData) {
        // Update the index
        this.moveToArray(data.index);
    }

    /// Called when destroyed.
    public destroyEntity(animated: boolean) {
        if (animated) {
            // Update state and animate the destroy
            this.state = EntityState.DestroyingAnimated;
            this.animateDestroy();
        } else {
            this.finishDestroy();
        }
    }

    /// Should be overriden in order to implement an animation. Should *not* call `super.animateDestroy()`, but instead
    /// call `this.finishDestroy()` when finished.
    public animateDestroy() {
        this.finishDestroy();
    }

    public finishDestroy() {
        // Update state
        this.id = -1;
        this.state = EntityState.PendingDestroy;
    }

    /// Called every frame.
    public update(dt: float) {

    }

    /// Moves to a position, called internally.
    public moveTo(x: int, y: int) {
        // Update the data
        this.x = x;
        this.y = y;

        // Move the entity if possible
        if (this.autoPosition) {
            this.position.set(Game.shared.indexToGlobalX(x), Game.shared.indexToGlobalY(y))
        }
    }

    /// Moves to a position array.
    public moveToArray(index: MapIndex) {
        this.moveTo(index[0], index[1]);
    }
}
