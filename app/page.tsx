'use client';

import { useEffect, useState } from 'react';
import neighborhoods from '../data/map.json';

type Reading = { temperature: number; time: string };
type Weather = { savedAt: number; readings: Record<string, Reading> };
const CACHE = 'fortaleza-weather-v1';
const HOUR = 3_600_000;
const color = (value?: number) => value === undefined ? '#deded7' : `hsl(${Math.max(8, Math.min(180, 180 - (value - 23) * 17))} 48% 65%)`;
const degrees = (value: number) => value.toLocaleString('pt-BR', { maximumFractionDigits: 1 });

export default function Home() {
  const [active, setActive] = useState<string | null>(null);
  const [weather, setWeather] = useState<Weather | null>(null);
  const [status, setStatus] = useState('Consultando temperaturas…');
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    async function update() {
      try {
        let cached: Weather | null = null;
        try { const raw = localStorage.getItem(CACHE); cached = raw ? JSON.parse(raw) : null; } catch { /* Storage is optional. */ }
        if (cached && typeof cached.savedAt === 'number' && cached.readings && neighborhoods.every(n => Number.isFinite(cached?.readings[n.id]?.temperature) && typeof cached?.readings[n.id]?.time === 'string')) {
          setWeather(cached);
          if (Date.now() - cached.savedAt < HOUR && cached.savedAt <= Date.now()) {
            setStatus(''); timer = setTimeout(update, HOUR - (Date.now() - cached.savedAt)); return;
          }
        }
        setStatus('Atualizando temperaturas…');
        const readings: Record<string, Reading> = {};
        // Batches keep request URLs short; each coordinate still counts toward the API quota.
        for (let start = 0; start < neighborhoods.length; start += 40) {
          const batch = neighborhoods.slice(start, start + 40);
          const params = new URLSearchParams({ latitude: batch.map(n => n.latitude.toFixed(6)).join(','), longitude: batch.map(n => n.longitude.toFixed(6)).join(','), current: 'temperature_2m', timezone: 'America/Fortaleza', forecast_days: '1' });
          const response = await fetch(`https://api.open-meteo.com/v1/forecast?${params}`, { signal: controller.signal });
          if (!response.ok) throw new Error(`HTTP ${response.status}`);
          const result = await response.json();
          const rows = Array.isArray(result) ? result : [result];
          if (rows.length !== batch.length) throw new Error('Resposta incompleta');
          batch.forEach((n, i) => {
            const current = rows[i]?.current;
            if (!Number.isFinite(current?.temperature_2m) || typeof current?.time !== 'string') throw new Error('Temperatura indisponível');
            readings[n.id] = { temperature: current.temperature_2m, time: current.time };
          });
        }
        if (controller.signal.aborted) return;
        const fresh = { savedAt: Date.now(), readings };
        setWeather(fresh); setStatus('');
        try { localStorage.setItem(CACHE, JSON.stringify(fresh)); } catch { /* Works without local storage. */ }
        timer = setTimeout(update, HOUR);
      } catch {
        if (!controller.signal.aborted) { setStatus('Não foi possível atualizar.'); timer = setTimeout(update, 5 * 60_000); }
      }
    }
    void update();
    return () => { controller.abort(); clearTimeout(timer); };
  }, [attempt]);
  const selected = neighborhoods.find(n => n.id === active);
  const reading = active ? weather?.readings[active] : undefined;
  return <main>
    <header><div className="brand"><span className="sun">☀</span><span>CLIMA LOCAL</span></div><div className="live"><i /> Atualização a cada hora</div></header>
    <section className="intro"><p className="eyebrow">CEARÁ, BRASIL</p><h1>Fortaleza<span>.</span></h1><p className="subtitle">Uma cidade, muitos climas.<br />Explore a temperatura em cada bairro.</p></section>
    <section className="map-area" aria-label="Mapa de temperaturas dos bairros de Fortaleza">
      <span className="ocean">OCEANO ATLÂNTICO</span><span className="north">N ↑</span>
      <svg viewBox="0 0 960 960" role="group" aria-label="121 bairros. Use Tab para explorar ou toque em um bairro." onMouseLeave={() => setActive(null)}>
        {neighborhoods.map(n => <path key={n.id} d={n.path} fill={color(weather?.readings[n.id]?.temperature)} fillRule="evenodd" tabIndex={0} role="button" aria-label={`${n.name}: ${weather?.readings[n.id] ? degrees(weather.readings[n.id].temperature) + ' graus Celsius' : 'temperatura indisponível'}`} aria-pressed={active === n.id} className={active === n.id ? 'selected' : ''} onMouseEnter={() => setActive(n.id)} onFocus={() => setActive(n.id)} onBlur={() => setActive(null)} onClick={() => setActive(n.id)} onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setActive(n.id); } if (e.key === 'Escape') setActive(null); }}><title>{n.name}</title></path>)}
      </svg>
      {selected && <aside className="tooltip" role="status"><span className="tooltip-label">{selected.name}</span><strong>{reading ? <>{degrees(reading.temperature)}<small>°C</small></> : '—'}</strong><span>{reading ? `Temperatura estimada · ${reading.time.slice(11, 16)}` : 'Temperatura indisponível'}</span></aside>}
      <div className="map-hint">{selected ? selected.name : 'Passe o mouse ou toque em um bairro'}</div>
    </section>
    <footer><div className="legend"><span>23°</span><div /><span>33°</span></div><p className="status" aria-live="polite">{status || (weather ? `Consultado às ${new Date(weather.savedAt).toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit', timeZone: 'America/Fortaleza' })}` : '')}{status.startsWith('Não') && <button onClick={() => setAttempt(a => a + 1)}>Tentar novamente</button>}</p><p className="credits">Temperaturas estimadas por modelo · <a href="https://open-meteo.com/" target="_blank" rel="noreferrer">Open-Meteo</a><br />Limites: <a href="https://mapas.fortaleza.ce.gov.br/mapa/21/bairros-de-fortaleza" target="_blank" rel="noreferrer">IPLANFOR</a> · 121 bairros</p></footer>
  </main>;
}
