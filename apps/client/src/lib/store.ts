import { create } from "zustand";
import { clearStoredSessionToken, loadSessionToken, storeSessionToken } from "./native";

export type SessionUser = { id: string; username: string; email?: string | null };

type SessionState = {
  token: string | null;
  user: SessionUser | null;
  hydrated: boolean;
  activeRoomId: string | null;
  hydrate: () => Promise<void>;
  setSession: (token: string, user: SessionUser) => void;
  clearSession: () => void;
  setActiveRoom: (roomId: string | null) => void;
};

const ACTIVE_ROOM_KEY = "opencade.active_room";

export const useSessionStore = create<SessionState>((set) => ({
  token: null,
  user: null,
  hydrated: false,
  activeRoomId: sessionStorage.getItem(ACTIVE_ROOM_KEY),
  hydrate: async () => {
    try {
      const token = await loadSessionToken();
      set({ token, hydrated: true });
    } catch {
      set({ token: null, user: null, hydrated: true });
    }
  },
  setSession: (token, user) => {
    set({ token, user });
    void storeSessionToken(token);
  },
  clearSession: () => {
    sessionStorage.removeItem(ACTIVE_ROOM_KEY);
    set({ token: null, user: null, activeRoomId: null });
    void clearStoredSessionToken();
  },
  setActiveRoom: (roomId) => {
    if (roomId) sessionStorage.setItem(ACTIVE_ROOM_KEY, roomId);
    else sessionStorage.removeItem(ACTIVE_ROOM_KEY);
    set({ activeRoomId: roomId });
  },
}));
