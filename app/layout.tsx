import type { Metadata } from 'next';
import './globals.css';
export const metadata: Metadata = { title: 'Fortaleza · Temperatura por bairro', description: 'Explore a temperatura estimada nos 121 bairros de Fortaleza.' };
export default function Layout({ children }: { children: React.ReactNode }) { return <html lang="pt-BR"><body>{children}</body></html>; }
