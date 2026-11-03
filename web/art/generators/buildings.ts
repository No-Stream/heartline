// Buildings are modelled once; each design supplies all four views from the same description.
import { drawBurgerStall } from "./building-burger-stall.ts";
import { drawParkGate } from "./building-park-gate.ts";
import { drawRestroom } from "./building-restroom.ts";
import { drawSouvenirShop } from "./building-souvenir-shop.ts";
import { drawTicketBooth } from "./building-ticket-booth.ts";
import type { Theme } from "./theme.ts";

export interface BuildingDesign {
  id: string;
  description: string;
  /** Tiles covered at rotation 0. */
  footprintX: number;
  footprintY: number;
  canvasWidth: number;
  canvasHeight: number;
  /** Height of the footprint centre from the top of the canvas. */
  anchorY: number;
  draw: (theme: Theme, view: number) => string;
}

export const BUILDINGS: BuildingDesign[] = [
  { id: "burger_stall", description: "Burger stall with a rooftop burger, a serving window, a menu sign and a staff door", footprintX: 2, footprintY: 2, canvasWidth: 148, canvasHeight: 176, anchorY: 134, draw: drawBurgerStall },
  { id: "restroom", description: "Restroom cottage with two marked doors, a flower box, a capped roof vent and a rear water tank", footprintX: 2, footprintY: 2, canvasWidth: 148, canvasHeight: 152, anchorY: 110, draw: drawRestroom },
  { id: "ticket_booth", description: "Round ticket booth with a cone roof, a cashier, posters and a rope barrier", footprintX: 2, footprintY: 2, canvasWidth: 148, canvasHeight: 158, anchorY: 116, draw: drawTicketBooth },
  { id: "souvenir_shop", description: "Half-timbered gift shop with a teddy in the window and balloons on the roof", footprintX: 3, footprintY: 3, canvasWidth: 208, canvasHeight: 256, anchorY: 196, draw: drawSouvenirShop },
  { id: "park_gate", description: "Park entrance: two towers joined by an arched banner reading WELCOME and, from behind, COME BACK", footprintX: 1, footprintY: 4, canvasWidth: 196, canvasHeight: 218, anchorY: 164, draw: drawParkGate },
];
