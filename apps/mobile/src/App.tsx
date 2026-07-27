import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useVirtualizer } from "@tanstack/react-virtual";
import "./App.css";

type TxnRow = {
  id: string;
  date: string;
  description: string;
  counterparty: string;
  origin: string;
  amount: string;
  balance: string;
  mode: string;
  bank: string;
};

type ImportError = { kind: string; message: string };

function isImportError(e: unknown): e is ImportError {
  return typeof e === "object" && e !== null && "kind" in e && "message" in e;
}

function describe(e: unknown): string {
  if (isImportError(e)) {
    switch (e.kind) {
      case "unsupported_bank":
        return "This bank isn't supported yet — only BOB, HDFC and ICICI statements work for now.";
      case "corrupt_pdf":
        return "That file doesn't look like a readable PDF statement.";
      case "malformed_statement":
        return `The statement couldn't be read fully (${e.message}).`;
      default:
        return e.message;
    }
  }
  return String(e);
}

const COLUMNS = [
  "Date",
  "Description",
  "Counterparty",
  "Transaction Origin",
  "Amount",
  "Balance",
  "Mode",
  "Bank",
];

export default function App() {
  const [rows, setRows] = useState<TxnRow[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  // Path of a picked PDF that turned out to be password-protected.
  const [pendingPath, setPendingPath] = useState<string | null>(null);
  const [password, setPassword] = useState("");
  const [pwWrong, setPwWrong] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setRows(await invoke<TxnRow[]>("list_transactions"));
    } catch (e) {
      setError(describe(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const doImport = async (path: string, pw?: string) => {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const count = await invoke<number>("import_statement", {
        path,
        password: pw ?? null,
      });
      setPendingPath(null);
      setPassword("");
      setPwWrong(false);
      setNotice(`Imported ${count} transaction${count === 1 ? "" : "s"}.`);
      await refresh();
    } catch (e) {
      if (isImportError(e) && e.kind === "wrong_password") {
        setPendingPath(path);
        setPwWrong(pw !== undefined);
      } else {
        setPendingPath(null);
        setPassword("");
        setPwWrong(false);
        setError(describe(e));
        console.error("import_statement failed:", e);
        // Best-effort: dump why detection/parsing failed to this window's
        // devtools console (F12) — the only visibility into a failed import
        // without shipping statement text anywhere off-device.
        if (isImportError(e) && e.kind !== "corrupt_pdf") {
          try {
            const diag = await invoke("diagnose_statement", { path, password: pw ?? null });
            console.error("statement diagnostics:", diag);
          } catch (diagErr) {
            console.error("diagnose_statement also failed:", diagErr);
          }
        }
      }
    } finally {
      setBusy(false);
    }
  };

  const pick = async () => {
    const picked = await open({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (typeof picked === "string") {
      await doImport(picked);
    }
  };

  const cancelPassword = () => {
    setPendingPath(null);
    setPassword("");
    setPwWrong(false);
  };

  const parentRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 36,
    overscan: 10,
  });

  return (
    <main className="app">
      <div className="toolbar">
        <button onClick={() => void pick()} disabled={busy || pendingPath !== null}>
          Pick statement PDF
        </button>
        {busy && <span className="status">Working…</span>}
      </div>

      {pendingPath !== null && (
        <form
          className="password-box"
          onSubmit={(e) => {
            e.preventDefault();
            if (password) void doImport(pendingPath, password);
          }}
        >
          <p>
            This statement is password-protected.
            {pwWrong && " That password didn't work — try again."}
          </p>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="Statement password"
            autoFocus
          />
          <button type="submit" disabled={busy || !password}>
            Unlock
          </button>
          <button type="button" onClick={cancelPassword} disabled={busy}>
            Cancel
          </button>
        </form>
      )}

      {error && <p className="error">{error}</p>}
      {notice && <p className="notice">{notice}</p>}

      <div className="table-scroll" ref={parentRef}>
        <div className="table">
          <div className="row head">
            {COLUMNS.map((c) => (
              <div className="cell" key={c}>
                {c}
              </div>
            ))}
          </div>
          {rows.length === 0 ? (
            <p className="empty">No transactions yet. Import a statement to get started.</p>
          ) : (
            <div
              style={{ height: virtualizer.getTotalSize(), position: "relative" }}
            >
              {virtualizer.getVirtualItems().map((v) => {
                const r = rows[v.index];
                return (
                  <div
                    className="row"
                    key={r.id}
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      height: v.size,
                      transform: `translateY(${v.start}px)`,
                    }}
                  >
                    <div className="cell">{r.date}</div>
                    <div className="cell">{r.description}</div>
                    <div className="cell">{r.counterparty}</div>
                    <div className="cell">{r.origin}</div>
                    <div className="cell num">{r.amount}</div>
                    <div className="cell num">{r.balance}</div>
                    <div className="cell">{r.mode}</div>
                    <div className="cell">{r.bank}</div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </main>
  );
}
