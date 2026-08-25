import { create } from "zustand";
import { clearStoredSessionToken, loadSessionToken, storeSessionToken } from "./native";

export type SessionUser = { id: string; username: string; email?: string | null };

type SessionState = {
  token: string | null;
  user: SessionUser | null;
  hydrated: boolean;
  hydrate: () => Promise<void>;
  setSession: (token: string, user: SessionUser) => void;
  clearSession: () => void;
};

export const useSessionStore = create<SessionState>((set) => ({
  token: null,
  user: null,
  hydrated: false,
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
    set({ token: null, user: null });
    void clearStoredSessionToken();
  },
}));
