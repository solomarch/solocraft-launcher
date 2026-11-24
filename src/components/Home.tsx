import { initials, loaderLabel } from "../lib/format";
import { isActive, useApp } from "../store/app";
import { Icon } from "./Icon";

export function Home() {
  const instances = useApp((s) => s.instances);
  const accounts = useApp((s) => s.accounts);
  const states = useApp((s) => s.states);
  const launch = useApp((s) => s.launch);
  const kill = useApp((s) => s.kill);
  const select = useApp((s) => s.select);
  const setPage = useApp((s) => s.setPage);
  const openModal = useApp((s) => s.openModal);

  const recent = [...instances].sort((a, b) => (b.lastPlayed ?? 0) - (a.lastPlayed ?? 0))[0];
  const running = recent ? isActive(states, recent.id) : false;

  return (
    <div className="page">
      {accounts.accounts.length === 0 && (
        <div className="banner">
          Add an account to play.
          <button className="btn" onClick={() => setPage("settings")}>
            Open settings
          </button>
        </div>
      )}

      {recent ? (
        <div className="hero">
          <div className="ico" style={{ background: recent.icon }}>
            {initials(recent.name)}
          </div>
          <div className="grow">
            <div className="kicker">{recent.lastPlayed ? "CONTINUE PLAYING" : "READY TO PLAY"}</div>
            <div className="t">{recent.name}</div>
            <div className="meta" style={{ margin: 0 }}>
              {loaderLabel(recent)}
            </div>
          </div>
          {running ? (
            <button className="btn danger big" onClick={() => kill(recent.id)}>
              <Icon name="stop" /> Stop
            </button>
          ) : (
            <button
              className="btn primary big"
              onClick={() => {
                void launch(recent.id);
                setPage("instances");
              }}
            >
              <Icon name="play" /> Play
            </button>
          )}
        </div>
      ) : (
        <div className="hero">
          <div className="grow">
            <div className="kicker">WELCOME</div>
            <div className="t">Create your first instance</div>
            <div className="meta" style={{ margin: 0 }}>
              Pick a Minecraft version and SoloCraft handles Java and game files for you.
            </div>
          </div>
          <button className="btn primary big" onClick={() => openModal("new")}>
            <Icon name="plus" /> Add instance
          </button>
        </div>
      )}

      {instances.length > 0 && (
        <>
          <div className="section-title">Your instances</div>
          <div className="shelf">
            {instances.map((i) => (
              <button
                key={i.id}
                className="tile"
                onClick={() => {
                  select(i.id);
                  setPage("instances");
                }}
              >
                {isActive(states, i.id) && <span className="dot" />}
                <div className="ico" style={{ background: i.icon }}>
                  {initials(i.name)}
                </div>
                <div className="nm">{i.name}</div>
                <div className="sub">{loaderLabel(i)}</div>
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
