import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useAttackerArrowPositions, type AttackerArrow } from "../useAttackerArrowPositions";

describe("useAttackerArrowPositions", () => {
  const callbacks = new Map<number, FrameRequestCallback>();
  let nextFrameId = 1;

  function flushFrames(count: number) {
    for (let i = 0; i < count; i++) {
      const frameCallbacks = Array.from(callbacks.entries());
      callbacks.clear();
      for (const [, callback] of frameCallbacks) {
        callback(performance.now());
      }
    }
  }

  beforeEach(() => {
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      const id = nextFrameId;
      nextFrameId += 1;
      callbacks.set(id, callback);
      return id;
    });
    vi.spyOn(window, "cancelAnimationFrame").mockImplementation((id) => {
      callbacks.delete(id);
    });
  });

  afterEach(() => {
    callbacks.clear();
    nextFrameId = 1;
    document.body.innerHTML = "";
    vi.restoreAllMocks();
  });

  it("remeasures attacker arrows after viewport resize once polling has stabilized", () => {
    const attacker = document.createElement("div");
    attacker.dataset.objectId = "1";
    const defenderHud = document.createElement("div");
    defenderHud.dataset.playerHud = "2";
    document.body.append(attacker, defenderHud);

    let attackerLeft = 10;
    let defenderLeft = 100;

    attacker.getBoundingClientRect = () => ({
      left: attackerLeft,
      top: 20,
      width: 20,
      height: 30,
      right: attackerLeft + 20,
      bottom: 50,
      x: attackerLeft,
      y: 20,
      toJSON: () => ({}),
    } as DOMRect);
    defenderHud.getBoundingClientRect = () => ({
      left: defenderLeft,
      top: 40,
      width: 40,
      height: 20,
      right: defenderLeft + 40,
      bottom: 60,
      x: defenderLeft,
      y: 40,
      toJSON: () => ({}),
    } as DOMRect);

    const arrows: AttackerArrow[] = [
      { attackerId: 1, target: { kind: "player", playerId: 2 }, isAtMe: true },
    ];

    const { result } = renderHook(() => useAttackerArrowPositions(arrows));

    act(() => flushFrames(11));
    expect(result.current).toEqual([
      {
        key: "1->p2",
        from: { x: 20, y: 35 },
        to: { x: 120, y: 50 },
        isAtMe: true,
      },
    ]);
    expect(callbacks.size).toBe(0);

    attackerLeft = 40;
    defenderLeft = 200;

    act(() => {
      window.dispatchEvent(new Event("resize"));
      flushFrames(1);
    });

    expect(result.current).toEqual([
      {
        key: "1->p2",
        from: { x: 50, y: 35 },
        to: { x: 220, y: 50 },
        isAtMe: true,
      },
    ]);
  });
});
