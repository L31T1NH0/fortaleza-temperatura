import fs from 'node:fs';
const geo = JSON.parse(fs.readFileSync(new URL('../data/bairros.geojson', import.meta.url)));
const project = ([lon, lat]) => [lon * Math.cos(-3.8 * Math.PI / 180), -Math.log(Math.tan(Math.PI / 4 + lat * Math.PI / 360)) * 180 / Math.PI];
const points = geo.features.flatMap(f => f.geometry.coordinates.flat(2)).map(project);
const minX = Math.min(...points.map(p=>p[0])), minY = Math.min(...points.map(p=>p[1]));
const scale = 900 / Math.max(Math.max(...points.map(p=>p[0]))-minX, Math.max(...points.map(p=>p[1]))-minY);
const xy = p => {const [x,y]=project(p);return [(x-minX)*scale+30,(y-minY)*scale+30];};
const inside = (p,ring) => {let yes=false;for(let i=0,j=ring.length-1;i<ring.length;j=i++){const a=ring[i],b=ring[j];if((a[1]>p[1])!==(b[1]>p[1])&&p[0]<(b[0]-a[0])*(p[1]-a[1])/(b[1]-a[1])+a[0])yes=!yes;}return yes;};
const neighborhoods=geo.features.map(f=>{
 const polygons=f.geometry.coordinates;
 const ring=polygons.reduce((a,b)=>a[0].length>b[0].length?a:b)[0];
 const lon0=Math.min(...ring.map(p=>p[0])),lon1=Math.max(...ring.map(p=>p[0])),lat0=Math.min(...ring.map(p=>p[1])),lat1=Math.max(...ring.map(p=>p[1]));
 let coord=[(lon0+lon1)/2,(lat0+lat1)/2];
 if(!polygons.some(poly=>inside(coord,poly[0])&&!poly.slice(1).some(h=>inside(coord,h)))){
  outer:for(let y=1;y<100;y++)for(let x=1;x<100;x++){const p=[lon0+(lon1-lon0)*x/100,lat0+(lat1-lat0)*y/100];if(polygons.some(poly=>inside(p,poly[0])&&!poly.slice(1).some(h=>inside(p,h)))){coord=p;break outer;}}
 }
 return {id:String(f.properties['Código do  Bairro']),name:f.properties.Nome,longitude:coord[0],latitude:coord[1],anchor:xy(coord),path:polygons.map(poly=>poly.map(r=>r.map((p,i)=>(i?'L':'M')+xy(p).map(v=>v.toFixed(2)).join(',')).join('')+'Z').join('')).join('')};
}).sort((a,b)=>a.name.localeCompare(b.name,'pt-BR'));
fs.writeFileSync(new URL('../data/map.json',import.meta.url),JSON.stringify(neighborhoods));
