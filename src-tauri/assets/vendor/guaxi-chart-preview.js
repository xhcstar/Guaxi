(function(){"use strict";const D=(e,t,i)=>{const n=e?.[t]??e?.[i];return n==null?"":String(n)},G=(e,t,i)=>{const n=e?.[t]??e?.[i];return n==null?[]:Array.isArray(n)?n.map(String):[String(n)]},f=e=>{if(e===""||e===null||e===void 0)return null;const t=Number(e);return Number.isFinite(t)?t:null},b=e=>e??"",$=(e,t={})=>e?.title?{text:String(e.title),...t}:void 0,P=(e,{seriesCount:t=1,names:i=[],extra:n={}}={})=>{if(e?.legend===!1||!(e?.legend===!0||t>1))return;const l=(Array.isArray(i)?i:[]).filter(Boolean);return{top:e?.title?42:0,type:"scroll",...l.length?{data:l}:{},...n}},c=(e,t,i={})=>({...e,...t?.seriesStyle&&typeof t.seriesStyle=="object"?t.seriesStyle:{},...i}),j=(e,{hasLegend:t=!1}={})=>{const i=(e?.title?52:16)+(t?28:0);return{left:Number(e?.gridLeft??52),right:Number(e?.gridRight??28),top:i,bottom:Number(e?.gridBottom??44),containLabel:!0}},A=(e,t,i)=>{const n={type:e?.xType==="time"||e?.xType==="value"?e.xType:"category",data:q(e,t,i),axisLabel:{hideOverlap:!0},boundaryGap:e?.boundaryGap!==void 0?!!e.boundaryGap:void 0};return e?.xName&&(n.name=String(e.xName)),e?.xAxisStyle&&typeof e.xAxisStyle=="object"&&Object.assign(n,e.xAxisStyle),n},q=(e,t,i)=>i?t.map(n=>b(n[i])):t.map((n,r)=>String(r+1)),k=(e,t="yAxisStyle")=>{const i={type:"value"};return e?.yName&&(i.name=String(e.yName)),e?.yMin!==void 0&&(i.min=e.yMin),e?.yMax!==void 0&&(i.max=e.yMax),e?.[t]&&typeof e[t]=="object"&&Object.assign(i,e[t]),i},_=(e,t="axis")=>{if(e?.tooltip!==!1)return e?.tooltip&&typeof e.tooltip=="object"?e.tooltip:{trigger:t}},K=e=>{if(e?.stack)return e.stack===!0?"total":String(e.stack)};function I(e,t,i,n){const r=String(e.type||"").toLowerCase()==="area",l=!!e.smooth,o=e.step?e.step===!0?"start":String(e.step):void 0;return n.map(s=>c({name:s,type:"line",data:t.map(u=>f(u[s])),smooth:l,...o?{step:o}:{},...r||e.areaStyle?{areaStyle:{}}:{},...e.connectNulls?{connectNulls:!0}:{},...e.symbol!==void 0?{symbol:e.symbol}:{},...e.lineWidth?{lineStyle:{width:Number(e.lineWidth)}}:{}},e))}function M(e,t,i,n){const r=K(e),l=e.orient==="horizontal"||e.horizontal===!0;return{series:n.map(a=>c({name:a,type:"bar",data:t.map(s=>f(s[a])),...r?{stack:r}:{},...e.showBackground?{showBackground:!0,backgroundStyle:{color:"rgba(180,180,180,0.12)"}}:{},...e.barWidth?{barWidth:e.barWidth}:{},...e.barGap?{barGap:e.barGap}:{},...e.barCategoryGap?{barCategoryGap:e.barCategoryGap}:{},...e.roundCap?{itemStyle:{borderRadius:[4,4,0,0]}}:{}},e)),horizontal:l}}function J(e,t,i,n){const r=n[0],l=t.map(g=>f(g[r])??0),o=t.map(g=>String(b(g[i]))),a=[],s=[],u=[];let d=0;l.forEach(g=>{g>=0?(a.push(d),s.push(g),u.push(0)):(a.push(d+g),s.push(0),u.push(-g)),d+=g}),e.total&&(o.push("合计"),a.push(0),s.push(d),u.push(0));const m={type:"bar",stack:"waterfall",barWidth:e.barWidth||"45%"};return{series:[c({...m,name:"_placeholder",itemStyle:{color:"transparent"},emphasis:{itemStyle:{color:"transparent"}},data:a},e),c({...m,name:"增加",itemStyle:{color:e.positiveColor||"#3ba272"},data:s},e),c({...m,name:"减少",itemStyle:{color:e.negativeColor||"#ee6666"},data:u},e)],extra:{xAxisData:o}}}const H=(e,{nameKey:t,minKey:i,maxKey:n,styleKey:r})=>{const l={type:"value"};return e?.[t]&&(l.name=String(e[t])),e?.[i]!==void 0&&(l.min=e[i]),e?.[n]!==void 0&&(l.max=e[n]),e?.[r]&&typeof e[r]=="object"&&Object.assign(l,e[r]),l};function W(e,t,i,n,{effect:r=!1}={}){const l=e.size?String(e.size):"",o=t.map(u=>{const d=[f(u[i])??f(u[n[0]]),f(u[n[1]])];return l&&d.push(f(u[l])??0),d}),a=Number(e.symbolSize||20);return{series:[c({name:e.title?String(e.title):"scatter",type:r?"effectScatter":"scatter",data:o,symbolSize:l?u=>{const d=Array.isArray(u)?Number(u[2]):0,m=Math.max(...o.map(g=>Number(g[2])||0),1);return Math.max(6,d/m*a)}:Number(e.symbolSize||12),...r?{showEffectOn:"render",rippleEffect:{brushType:e.rippleType||"stroke",scale:e.rippleScale||3}}:{}},e)],xAxis:H(e,{nameKey:"xName",minKey:"xMin",maxKey:"xMax",styleKey:"xAxisStyle"}),yAxis:k(e)}}function U(e,t,i,n){const r=Array.isArray(e.fields)&&e.fields.length>=4?e.fields.map(String):n.length>=4?n.slice(0,4):["open","close","low","high"],l=t.map(o=>r.map(a=>f(o[a])??0));return[c({name:e.title?String(e.title):"K线",type:"candlestick",data:l,itemStyle:{color:e.riseColor||"#ef5350",color0:e.fallColor||"#26a69a",borderColor:e.riseColor||"#ef5350",borderColor0:e.fallColor||"#26a69a"}},e)]}function Y(e,t,i,n){const r=Array.isArray(e.fields)&&e.fields.length>=5?e.fields.map(String):n;let l;return r.length>=5?l=t.map(o=>r.slice(0,5).map(a=>f(o[a])??0)):l=t.map(o=>{const a=Object.values(o).map(f).filter(u=>u!==null).sort((u,d)=>u-d);if(!a.length)return[0,0,0,0,0];const s=u=>a[Math.min(a.length-1,Math.floor((a.length-1)*u))];return[a[0],s(.25),s(.5),s(.75),a[a.length-1]]}),[c({name:e.title?String(e.title):"箱线",type:"boxplot",data:l},e)]}function Z(e,t,i,n){const r=[...new Set(t.map(s=>String(b(s[i]))))],l=n.length>=2?[...new Set(t.map(s=>String(b(s[n[0]]))))]:[String(e.yLabel||"值")],o=t.map(s=>{const u=r.indexOf(String(b(s[i]))),d=n.length>=2?l.indexOf(String(b(s[n[0]]))):0,m=f(s[n.length>=2?n[1]:n[0]])??0;return[u,d,m]}),a=o.map(s=>s[2]);return{series:c({name:e.title?String(e.title):"热力",type:"heatmap",data:o},e),xAxis:{type:"category",data:r,splitArea:{show:!0}},yAxis:{type:"category",data:l,splitArea:{show:!0}},visualMap:{min:Math.min(...a,0),max:Math.max(...a,1),calculable:!0,orient:"horizontal",left:"center",bottom:0,inRange:{color:e.colorRange||["#e0f3ff","#0f6fd6"]}}}}function Q(e,t,i,n){const r=e.symbol||"circle";return n.map(l=>c({name:l,type:"pictorialBar",symbol:r,symbolRepeat:e.symbolRepeat!==!1,symbolSize:e.symbolSize||[16,8],symbolMargin:e.symbolMargin||"10%",symbolClip:!0,data:t.map(o=>f(o[l]))},e))}function X(e,t,i,n){const r=e.name?String(e.name):n[1]||"",l=n[0],o=t.map(a=>[String(b(a[i])),f(a[l])??0,String(b(a[r]))]);return[c({name:e.title?String(e.title):"河流",type:"themeRiver",data:o,label:{show:!!e.showLabel}},e)]}function V(e,t,i,n){const r=String(e.variant||"gantt").toLowerCase();if(n[0],r==="gantt"){const l=e.start?String(e.start):n[0],o=e.end?String(e.end):n[1]||n[0],a=t.map(u=>String(b(u[i]))),s=t.map((u,d)=>({name:a[d],value:[d,f(u[l])??0,f(u[o])??0],itemStyle:{color:e.barColor||"#5470c6"}}));return{series:[c({type:"custom",renderItem:ee,encode:{x:[1,2],y:0},data:s},e)],xAxis:{type:"value",min:e.axisMin,max:e.axisMax},yAxis:{type:"category",data:a,inverse:!0}}}if(r==="bullet"||r==="progress"){const l=e.goal!=null?Number(e.goal):100;return n.map(o=>c({name:o,type:"bar",data:t.map(a=>f(a[o])),barWidth:e.barWidth||14},e)).concat([c({name:"目标",type:"bar",data:t.map(()=>l),barWidth:e.barWidth||14,barGap:"-100%",itemStyle:{color:"rgba(0,0,0,0.08)"}},e)])}return M(e,t,i,n).series}function ee(e,t){const i=t.value(0),n=t.coord([t.value(1),i]),r=t.coord([t.value(2),i]),l=t.size([0,1])[1]*.5;return{type:"rect",shape:{x:n[0],y:n[1]-l/2,width:Math.max(2,r[0]-n[0]),height:l},style:{fill:e?.style?.fill??e?.itemStyle?.color??"#5470c6"},emphasis:{style:{stroke:"#000"}}}}function te(e,t,i,n){const r=e.parallelFields?e.parallelFields.map(String):n;return{series:c({name:e.title?String(e.title):"平行坐标",type:"parallel",data:t.map(l=>r.map(o=>f(l[o])??0)),lineStyle:{width:1.5,opacity:.55}},e),parallelAxis:r.map((l,o)=>({dim:o,name:l,...Array.isArray(e.parallelRanges)&&e.parallelRanges[o]?{min:e.parallelRanges[o][0],max:e.parallelRanges[o][1]}:{}}))}}function N(e,t,i,n){const r=i||Object.keys(t[0]||{})[0],l=n[0]||Object.keys(t[0]||{})[1],o=String(e.type||"").toLowerCase(),a=e.roseType===!0||e.roseType==="angle"||e.roseType==="radius",s=e.radius??(o==="pie"&&!a?"62%":["42%","68%"]);return[c({type:"pie",radius:s,center:e.center||["50%","55%"],...a?{roseType:e.roseType===!0?"radius":String(e.roseType)}:{},data:t.map(d=>({name:String(b(d[r]??"")),value:Number(f(d[l]))||0})),label:{formatter:e.labelFormatter||"{b}: {d}%",...e.showLabel===!1?{show:!1}:{}},...e.donutTitle?{emphasis:{scale:!0}}:{}},e)]}function ne(e,t,i,n){const r=n.map(o=>{const a=t.map(u=>f(u[o])??0),s=e.max!=null?Number(e.max):Math.max(...a,1)*1.15;return{name:o,max:s}});return{series:[c({type:"radar",data:t.map(o=>({name:i?String(b(o[i])):"",value:n.map(a=>f(o[a])??0)})),...e.areaStyle!==!1?{areaStyle:{opacity:.18}}:{}},e)],radar:{indicator:r,...e.radarRadius?{radius:e.radarRadius}:{},splitArea:{areaStyle:{color:["rgba(250,250,250,0.1)","rgba(200,200,200,0.08)"]}}}}}function ie(e,t,i,n){const r=n[0],l=e.max!=null?Number(e.max):Math.max(...t.map(y=>f(y[r])??0),100);if(t.length===1&&!e.multi){const y=Number(e.detailFontSize||22);return[c({type:"gauge",min:e.min!=null?Number(e.min):0,max:l,progress:e.progress!==!1?{show:!0,width:Number(e.progressWidth||12)}:void 0,axisLine:{lineStyle:{width:Number(e.axisLineWidth||12)}},pointer:e.pointer===!1?{show:!1}:void 0,detail:{formatter:e.formatter||"{value}",fontSize:y,lineHeight:Math.round(y*1.25),offsetCenter:[0,"44%"]},title:{offsetCenter:[0,"78%"],fontSize:12},data:[{value:f(t[0][r])??0,name:i?String(b(t[0][i])):""}]},e)]}const o=e.legend===!0||e.legend!==!1&&t.length>1,a=Number(e.baseRadius||45),s=Number(e.ringGap||12),u=o&&t.length>1?Math.min(s,Math.max(5,(72-a)/(t.length-1))):s,d=t.length,m=16,g=-((d-1)*m)/2;return{series:t.map((y,h)=>c({type:"gauge",name:i?String(b(y[i])):"",min:e.min!=null?Number(e.min):0,max:l,radius:`${Math.round(a+h*u)}%`,startAngle:90,endAngle:-270,pointer:{show:!1},progress:{show:!0,overlap:!1,roundCap:!0,clip:!1},axisLine:{lineStyle:{width:10}},splitLine:{show:!1},axisTick:{show:!1},axisLabel:{show:!1},title:{show:!!i,fontSize:12,offsetCenter:[0,g+h*m]},detail:{show:!1},data:[{value:f(y[r])??0,name:i?String(b(y[i])):""}]},e)),legend:o?{bottom:0,type:"scroll"}:void 0}}function re(e,t,i,n){const r=i||Object.keys(t[0]||{})[0],l=n[0]||Object.keys(t[0]||{})[1],o=e.sort===!1?t:[...t].sort((a,s)=>(f(s[l])??0)-(f(a[l])??0));return[c({type:"funnel",left:e.left||"12%",right:e.right||"12%",top:e.top||40,bottom:e.bottom||24,sort:e.sort||"descending",gap:Number(e.gap??2),label:{show:!0,position:"inside",formatter:e.labelFormatter||"{b}: {c}"},data:o.map(a=>({name:String(b(a[r]??"")),value:Number(f(a[l]))||0}))},e)]}function ae(e,t,i,n){const r=e.source?String(e.source):i,l=e.target?String(e.target):n[0],o=e.value?String(e.value):n[1]||n[0],a=new Set,s=t.map(u=>{const d=String(b(u[r])),m=String(b(u[l]));return d&&a.add(d),m&&a.add(m),{source:d,target:m,value:f(u[o])??0}});return[c({type:"sankey",data:[...a].map(u=>({name:u})),links:s,emphasis:{focus:"adjacency"},lineStyle:{curveness:Number(e.curveness??.5),opacity:Number(e.linkOpacity??.4)},label:{fontSize:12},nodeGap:Number(e.nodeGap??12),nodeWidth:Number(e.nodeWidth??16)},e)]}function le(e,t,i,n){const r=C(e,t,i,n);return[c({type:"tree",data:[r],layout:e.layout||"orthogonal",orient:e.orient||"LR",symbolSize:Number(e.symbolSize||7),initialTreeDepth:Number(e.depth??3),label:{position:"left",verticalAlign:"middle",align:"right",fontSize:12},leaves:{label:{position:"right",align:"left"}},expandAndCollapse:e.collapsible!==!1,roam:e.roam===!0,top:e.top||"4%",bottom:e.bottom||"4%"},e)]}function C(e,t,i,n){const r=e.rootName?String(e.rootName):"root",l={name:r,children:[]},o=new Map([[r,l]]),a=(S,y)=>{let h=l;for(const p of S){if(!p)continue;let w=(h.children||[]).find(O=>O.name===p);w||(w={name:p,children:[]},h.children=h.children||[],h.children.push(w)),h=w}y!=null&&(h.value=y)},s=e.path?String(e.path):"",u=e.parent?String(e.parent):"",d=e.id?String(e.id):"",m=n[0];for(const S of t)if(s)a(String(b(S[s])).split("/").map(y=>y.trim()),f(S[m]));else if(u&&d){const y=String(b(S[u]))||r,h=String(b(S[d]));o.has(y)||a([y],null),a([y===r?"":y,h].filter(Boolean),f(S[m]))}else i&&a([String(b(S[i]))],f(S[m]));const g=S=>(S.children&&S.children.length===0?delete S.children:S.children?.forEach(g),S);return g(l)}function oe(e,t,i,n){const r=C(e,t,i,n);return[c({type:"treemap",data:r.children||[],leafDepth:e.leafDepth!=null?Number(e.leafDepth):void 0,roam:e.roam===!0,label:{show:!0,formatter:"{b}"},upperLabel:{show:!0,height:20},itemStyle:{borderColor:"#fff",borderWidth:1,gapWidth:1},levels:e.levels||void 0},e)]}function se(e,t,i,n){const r=C(e,t,i,n);return[c({type:"sunburst",data:r.children||[],radius:e.radius||[0,"88%"],label:{rotate:"radial",fontSize:11},itemStyle:{borderColor:"#fff",borderWidth:1},emphasis:{focus:"ancestor"}},e)]}function ue(e,t,i,n){const r=e.source?String(e.source):i,l=e.target?String(e.target):n[0],o=e.value?String(e.value):n[1],a=new Map,s=[];for(const u of t){const d=String(b(u[r])),m=String(b(u[l]));if(d&&!a.has(d)&&a.set(d,{name:d,value:0}),m&&!a.has(m)&&a.set(m,{name:m,value:0}),d&&m){const g=o?f(u[o])??1:1;s.push({source:d,target:m,value:g}),a.has(d)&&(a.get(d).value+=1),a.has(m)&&(a.get(m).value+=1)}}return[c({type:"graph",layout:e.layout||"force",data:[...a.values()],links:s,force:{repulsion:Number(e.repulsion||180),edgeLength:Number(e.edgeLength||90),gravity:.1},roam:e.roam!==!1,label:{show:!0,fontSize:11},lineStyle:{color:"source",curveness:Number(e.curveness??.12),opacity:.55},emphasis:{focus:"adjacency",lineStyle:{width:3}},categories:e.categories||void 0,symbolSize:Number(e.symbolSize||14)},e)]}function de(e,t,i,n){const r=Array.isArray(e.fields)&&e.fields.length>=4?e.fields.map(String):n,l=t.map(o=>{const a=(r.length>=4?r.slice(0,4):Object.keys(o).slice(0,4)).map(s=>f(o[s])??0);return{coords:[[a[0],a[1]],[a[2],a[3]]]}});return[c({type:"lines",coordinateSystem:e.coordinateSystem||"cartesian2d",polyline:e.polyline===!0,data:l,effect:e.effect===!1?void 0:{show:!0,period:Number(e.period||4),trailLength:.2,symbolSize:4},lineStyle:{curveness:Number(e.curveness??.3),width:Number(e.lineWidth||1.2),opacity:.7}},e)]}function fe(e,t,i,n){const r=e.map?String(e.map):"";if(!r)throw new Error("map 类型需要在 spec.map 里指定已注册的地图名。离线环境不内置地理数据，请在笔记同目录放一份 GeoJSON，或改用 province/city 名称 + type: bar 的替代方案。");const l=n[0],o=i||"name";return[c({type:"map",map:r,roam:e.roam!==!1,data:t.map(a=>({name:String(b(a[o])),value:f(a[l])??0})),label:{show:!!e.showLabel,fontSize:10},emphasis:{label:{show:!0}}},e)]}function T(e,t,i,n){const r=n[0],l=f(t[0]?.[r])??0,o=e.max?l/Number(e.max):l>1?l/100:l;return[c({type:"liquidFill",data:[Number(o.toFixed(4)),Number((o*.82).toFixed(4))],radius:e.radius||"72%",outline:{show:e.outline!==!1},label:{formatter:e.formatter||`${Math.round(o*100)}%`,fontSize:Number(e.detailFontSize||26)},color:e.colors||["#3a8ee6","#64a9ec"],backgroundStyle:{color:"transparent"}},e)]}function R(e,t,i,n){const r=i||Object.keys(t[0]||{})[0],l=n[0]||Object.keys(t[0]||{})[1];return[c({type:"wordCloud",shape:e.shape||"circle",left:"center",top:"center",width:e.width||"92%",height:e.height||"88%",sizeRange:e.sizeRange||[14,64],rotationRange:e.rotationRange||[-45,45],gridSize:Number(e.gridSize||10),drawOutOfBound:!1,textStyle:{fontFamily:"sans-serif",fontWeight:"bold",color:void 0},data:t.map(o=>({name:String(b(o[r])),value:f(o[l])??1}))},e)]}const me={liquidfill:{builder:T,requires:"echarts-liquidfill"},wordcloud:{builder:R,requires:"echarts-wordcloud"}},v={line:{family:"cartesian",builder:(e,t,i,n)=>I(e,t,i,n)},area:{family:"cartesian",builder:(e,t,i,n)=>I(e,t,i,n)},bar:{family:"cartesian",builder:(e,t,i,n)=>M(e,t,i,n).series},waterfall:{family:"cartesian",builder:(e,t,i,n)=>J(e,t,i,n)},scatter:{family:"cartesian",builder:(e,t,i,n)=>W(e,t,i,n)},effectscatter:{family:"cartesian",builder:(e,t,i,n)=>W(e,t,i,n,{effect:!0})},boxplot:{family:"cartesian",builder:(e,t,i,n)=>Y(e,t,i,n)},candlestick:{family:"cartesian",builder:(e,t,i,n)=>U(e,t,i,n)},heatmap:{family:"independent",builder:(e,t,i,n)=>Z(e,t,i,n)},pictorialbar:{family:"cartesian",builder:(e,t,i,n)=>Q(e,t,i,n)},themeriver:{family:"cartesian",builder:(e,t,i,n)=>X(e,t,i,n)},custom:{family:"custom",builder:(e,t,i,n)=>V(e,t,i,n)},pie:{family:"independent",builder:(e,t,i,n)=>N(e,t,i,n)},doughnut:{family:"independent",builder:(e,t,i,n)=>N(e,t,i,n)},ring:{family:"independent",builder:(e,t,i,n)=>N(e,t,i,n)},radar:{family:"independent",builder:(e,t,i,n)=>ne(e,t,i,n)},parallel:{family:"independent",builder:(e,t,i,n)=>te(e,t,i,n)},sankey:{family:"independent",builder:(e,t,i,n)=>ae(e,t,i,n)},funnel:{family:"independent",builder:(e,t,i,n)=>re(e,t,i,n)},gauge:{family:"independent",builder:(e,t,i,n)=>ie(e,t,i,n)},tree:{family:"independent",builder:(e,t,i,n)=>le(e,t,i,n)},treemap:{family:"independent",builder:(e,t,i,n)=>oe(e,t,i,n)},sunburst:{family:"independent",builder:(e,t,i,n)=>se(e,t,i,n)},graph:{family:"independent",builder:(e,t,i,n)=>ue(e,t,i,n)},map:{family:"independent",builder:(e,t,i,n)=>fe(e,t,i,n)},lines:{family:"independent",builder:(e,t,i,n)=>de(e,t,i,n)},liquidfill:{family:"extension",builder:(e,t,i,n)=>T(e,t,i,n)},wordcloud:{family:"extension",builder:(e,t,i,n)=>R(e,t,i,n)}},L={kline:"candlestick",candlestick:"candlestick",violin:"boxplot",waterful:"waterfall",waterfallchart:"waterfall",donut:"doughnut",circle:"pie",rosetype:"pie",bubble:"scatter",scatter3d:"scatter",force:"graph",network:"graph",wordcloud:"wordcloud",liquid:"liquidfill",liquidfill:"liquidfill"};function E(e){const t=String(e||"line").trim().toLowerCase().replace(/[\s_-]/g,"");return v[t]?t:L[t]&&v[L[t]]?L[t]:""}function B(){return Object.keys(v).sort()}function ce(e,t=","){const i=[];let n=[],r="",l=!1;const o=String(e||"").replace(/^﻿/,"");for(let a=0;a<o.length;a+=1){const s=o[a];if(l){s==='"'?o[a+1]==='"'?(r+='"',a+=1):l=!1:r+=s;continue}s==='"'?l=!0:s===t?(n.push(r),r=""):s===`
`?(n.push(r),i.push(n),n=[],r=""):s==="\r"||(r+=s)}return(r.length||n.length)&&(n.push(r),i.push(n)),i.filter(a=>a.some(s=>String(s).trim()!==""))}const ge=e=>{const t=String(e??"").trim();return t===""?"":/^-?\d+(\.\d+)?([eE][+-]?\d+)?$/.test(t)?Number(t):t};function ye(e,t=","){const i=ce(e,t);if(i.length<2)return[];const n=i[0].map((r,l)=>String(r).trim()||`col${l+1}`);return i.slice(1).map(r=>{const l={};return n.forEach((o,a)=>{l[o]=ge(r[a])}),l})}function he(e,t=""){const i=String(e||"").trim();if(!i)return[];const n=/^[[{]/.test(i),r=String(t||"").toLowerCase();if(n||r==="json")try{const l=JSON.parse(i);return Array.isArray(l)?l:l&&Array.isArray(l.data)?l.data:l&&Array.isArray(l.rows)?l.rows:l&&typeof l=="object"?[l]:[]}catch(l){if(n)throw new Error(`数据不是合法 JSON：${l.message}`)}return ye(i,r==="tsv"?"	":",")}function be(e){const t={},i=String(e||"").split(/\r?\n/);let n="",r=!1;const l=o=>{const a=String(o||"").trim();if(!a)return"";if(a==="true")return!0;if(a==="false")return!1;if(a==="null")return null;if(/^-?\d+(\.\d+)?([eE][+-]?\d+)?$/.test(a))return Number(a);if(/^[[{]/.test(a))try{return JSON.parse(a)}catch{return a.startsWith("[")&&a.endsWith("]")?a.slice(1,-1).split(",").map(s=>s.trim().replace(/^['"]|['"]$/g,"")).filter(Boolean):a}return/^(['"]).*\1$/.test(a)?a.slice(1,-1):a};for(const o of i){const a=o.trim();if(!a||a.startsWith("#"))continue;const s=a.match(/^-\s+(.*)$/);if(s&&n){r||(t[n]=[],r=!0),Array.isArray(t[n])&&t[n].push(l(s[1]));continue}const u=a.match(/^([A-Za-z_][\w-]*)\s*:\s*(.*)$/);if(!u)continue;const[,d,m]=u,g=m.trim();if(!g){n=d,r=!1,t[d]=t[d]??null;continue}n="",r=!1,t[d]=l(g)}for(const[o,a]of Object.entries(t))a===null&&(t[o]=[]);return t}function Se(e){const t=String(e||""),i={spec:null,specRaw:"",data:"",dataFormat:"",dataInline:!1},n=t.trim();if(/^\s*\{/.test(n))try{return i.spec=JSON.parse(n),i.specRaw=n,i}catch{}const r=t.split(/\r?\n/);let l="";const o=[],a=[];let s="";for(const d of r){const m=d.trim();if(!m){l==="data"?a.push(""):l==="spec"&&o.push("");continue}const g=m.match(/^(spec|data)\s*:\s*(.*)$/i);if(g&&!/^\s/.test(d)){const[,S,y]=g;if(l=S.toLowerCase(),l==="data"){const h=y.trim();h&&h!=="|"&&h!==">"&&(/^resource:/i.test(h)||/^bundled:/i.test(h)||/^\.{0,2}\//.test(h)||/^[a-zA-Z]:[\\/]/.test(h)||/\.(csv|tsv|json)$/i.test(h)?s=h:(a.push(h),i.dataInline=!0))}continue}l==="data"?(a.push(d),i.dataInline=!0):l==="spec"&&o.push(d)}const u=o.join(`
`).trim();if(u){i.specRaw=u;try{i.spec=JSON.parse(u)}catch{try{i.spec=JSON.parse(`{${u}}`)}catch{const d=be(u);i.spec=Object.keys(d).length?d:null}}}return s?i.data=s:a.length&&(i.data=a.join(`
`).trim()),i.data&&!i.dataInline?i.dataFormat="auto":i.data&&(i.dataFormat=/^[[{]/.test(i.data)?"json":i.data.includes("	")?"tsv":"csv"),i}function xe(e,t){if(!e||typeof e!="object")throw new Error('缺少 spec（示例：spec: {"type":"bar","x":"month","y":["revenue"]}）');if(e.option&&typeof e.option=="object"||Array.isArray(e.series)||e.xAxis!==void 0||e.yAxis!==void 0){const x=e.option&&typeof e.option=="object"?{...e.option}:{...e};return!x.dataset&&t.length&&(x.dataset={source:t}),x}const n=E(e.type);if(!n)throw new Error(`不支持的图表类型「${e.type}」。可用类型：${B().join(" / ")}`);const r=v[n],l=D(e,"x","xField"),o=G(e,"y","yField"),a=$(e),s=r.builder(e,t,l,o);let u,d=null;if(Array.isArray(s))u=s;else if(s&&typeof s=="object"&&s.series!==void 0)d=s,u=Array.isArray(s.series)?s.series:[s.series];else if(s&&typeof s=="object")u=[s];else throw new Error("图表构造器没有返回任何 series");const m=Array.isArray(u)?u:[u],g=m.length,S=m.map(x=>x&&typeof x=="object"?String(x.name||"").trim():"").filter(x=>x&&!x.startsWith("_")),y=P(e,{seriesCount:g,names:S}),h=_(e,r.family==="cartesian"?"axis":"item");if(r.family==="independent"||r.family==="extension")return{title:a,tooltip:h,legend:y,series:u,...d?{...d,series:u}:{}};if(r.family==="custom")return{title:a,tooltip:h,legend:y,series:u,...d||{},...d?.xAxis?{}:{xAxis:A(e,t,l),yAxis:k(e)},...d?.grid?{}:{grid:j(e,{hasLegend:!!y})}};const p=d?.horizontal===!0||e.orient==="horizontal"||e.horizontal===!0,w=d?.xAxis??(p?k(e):A(e,t,l)),O=d?.yAxis??(p?A(e,t,l):k(e));return{title:a,tooltip:h,legend:y,grid:d?.grid??j(e,{hasLegend:!!y}),xAxis:w,yAxis:O,...d?.visualMap?{visualMap:d.visualMap}:{},series:u}}function pe(e){if(!e||typeof e!="object")return e;const t={...e};t.backgroundColor||(t.backgroundColor="#ffffff"),t.title&&typeof t.title=="object"&&!t.title.textStyle?.color&&(t.title={...t.title,textStyle:{color:"#1f2328",...t.title.textStyle||{}}}),t.legend&&typeof t.legend=="object"&&!t.legend.textStyle?.color&&(t.legend={...t.legend,textStyle:{color:"#4a4f57",...t.legend.textStyle||{}}});const i=n=>{if(!n||typeof n!="object")return n;const r={...n};return r.axisLabel={color:"#4a4f57",...r.axisLabel||{}},r.axisLine={...r.axisLine||{},lineStyle:{color:"#c9ced6",...r.axisLine?.lineStyle||{}}},r.splitLine&&(r.splitLine={...r.splitLine,lineStyle:{color:"#eef0f3",...r.splitLine?.lineStyle||{}}}),r.name&&!r.nameTextStyle?.color&&(r.nameTextStyle={color:"#4a4f57",...r.nameTextStyle||{}}),r};return Array.isArray(t.xAxis)?t.xAxis=t.xAxis.map(i):t.xAxis&&(t.xAxis=i(t.xAxis)),Array.isArray(t.yAxis)?t.yAxis=t.yAxis.map(i):t.yAxis&&(t.yAxis=i(t.yAxis)),t}const we=["charts/stock.csv","charts/2026/10/sales.csv","boards/architecture.excalidraw"],ve=e=>/^bundled:/i.test(String(e||"").trim()),z=e=>String(e||"").replace(/^bundled:/i,"").replace(/\\/g,"/").replace(/^\.?\/*/,"").trim().toLowerCase();function ke(e){const t=new Map;for(const[i,n]of Object.entries(e||{}))t.set(z(i),n);return{list:()=>we.filter(i=>t.has(z(i))),get(i){const n=z(i),r=t.get(n);return r?{text:String(r.text||""),format:String(r.format||""),key:n}:null}}}const F=ke({"charts/stock.csv":{text:`date,close
2026-09-01,32.18
2026-09-02,32.64
2026-09-03,31.97
2026-09-04,32.45
2026-09-05,33.10
2026-09-08,33.62
2026-09-09,33.04
2026-09-10,32.71
2026-09-11,33.28
2026-09-12,34.05
2026-09-15,34.42
2026-09-16,33.86
2026-09-17,34.20
2026-09-18,35.11
2026-09-19,35.47
2026-09-22,34.93
2026-09-23,35.28
2026-09-24,36.05
2026-09-25,35.74
2026-09-26,36.38
2026-09-29,37.02
2026-09-30,36.55
2026-10-08,37.46
2026-10-09,38.12
`,format:"csv"},"charts/2026/10/sales.csv":{text:`month,revenue,cost
一月,120,40
二月,180,60
三月,160,55
四月,210,72
五月,195,66
六月,240,80
七月,228,75
八月,262,88
九月,305,96
十月,288,90
十一月,330,104
十二月,376,118
`,format:"csv"},"boards/architecture.excalidraw":{text:`{
  "type": "excalidraw",
  "version": 2,
  "source": "https://excalidraw.com",
  "elements": [
    {
      "id": "guaxi-sample-title",
      "type": "text",
      "x": 160,
      "y": 32,
      "width": 240,
      "height": 35,
      "angle": 0,
      "strokeColor": "#1e1e1e",
      "backgroundColor": "transparent",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "solid",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "roundness": null,
      "seed": 1001,
      "version": 1,
      "versionNonce": 1001,
      "isDeleted": false,
      "boundElements": null,
      "updated": 1,
      "link": null,
      "locked": false,
      "text": "示例架构",
      "fontSize": 28,
      "fontFamily": 1,
      "textAlign": "center",
      "verticalAlign": "top",
      "containerId": null,
      "originalText": "示例架构",
      "lineHeight": 1.25,
      "baseline": 25
    },
    {
      "id": "guaxi-sample-client",
      "type": "rectangle",
      "x": 60,
      "y": 130,
      "width": 160,
      "height": 80,
      "angle": 0,
      "strokeColor": "#1971c2",
      "backgroundColor": "#d0ebff",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "solid",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "roundness": { "type": 3 },
      "seed": 1002,
      "version": 1,
      "versionNonce": 1002,
      "isDeleted": false,
      "boundElements": [
        { "type": "text", "id": "guaxi-sample-client-text" }
      ],
      "updated": 1,
      "link": null,
      "locked": false
    },
    {
      "id": "guaxi-sample-client-text",
      "type": "text",
      "x": 96,
      "y": 158,
      "width": 88,
      "height": 25,
      "angle": 0,
      "strokeColor": "#1971c2",
      "backgroundColor": "transparent",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "solid",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "roundness": null,
      "seed": 1003,
      "version": 1,
      "versionNonce": 1003,
      "isDeleted": false,
      "boundElements": null,
      "updated": 1,
      "link": null,
      "locked": false,
      "text": "客户端",
      "fontSize": 20,
      "fontFamily": 1,
      "textAlign": "center",
      "verticalAlign": "middle",
      "containerId": "guaxi-sample-client",
      "originalText": "客户端",
      "lineHeight": 1.25,
      "baseline": 18
    },
    {
      "id": "guaxi-sample-server",
      "type": "rectangle",
      "x": 300,
      "y": 130,
      "width": 160,
      "height": 80,
      "angle": 0,
      "strokeColor": "#2f9e44",
      "backgroundColor": "#d3f9d8",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "solid",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "roundness": { "type": 3 },
      "seed": 1004,
      "version": 1,
      "versionNonce": 1004,
      "isDeleted": false,
      "boundElements": [
        { "type": "text", "id": "guaxi-sample-server-text" }
      ],
      "updated": 1,
      "link": null,
      "locked": false
    },
    {
      "id": "guaxi-sample-server-text",
      "type": "text",
      "x": 336,
      "y": 158,
      "width": 88,
      "height": 25,
      "angle": 0,
      "strokeColor": "#2f9e44",
      "backgroundColor": "transparent",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "solid",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "roundness": null,
      "seed": 1005,
      "version": 1,
      "versionNonce": 1005,
      "isDeleted": false,
      "boundElements": null,
      "updated": 1,
      "link": null,
      "locked": false,
      "text": "服务端",
      "fontSize": 20,
      "fontFamily": 1,
      "textAlign": "center",
      "verticalAlign": "middle",
      "containerId": "guaxi-sample-server",
      "originalText": "服务端",
      "lineHeight": 1.25,
      "baseline": 18
    },
    {
      "id": "guaxi-sample-arrow",
      "type": "arrow",
      "x": 226,
      "y": 170,
      "width": 68,
      "height": 0,
      "angle": 0,
      "strokeColor": "#1e1e1e",
      "backgroundColor": "transparent",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "solid",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "roundness": { "type": 2 },
      "seed": 1006,
      "version": 1,
      "versionNonce": 1006,
      "isDeleted": false,
      "boundElements": null,
      "updated": 1,
      "link": null,
      "locked": false,
      "points": [
        [0, 0],
        [68, 0]
      ],
      "lastCommittedPoint": null,
      "startBinding": {
        "elementId": "guaxi-sample-client",
        "focus": 0,
        "gap": 6
      },
      "endBinding": {
        "elementId": "guaxi-sample-server",
        "focus": 0,
        "gap": 6
      },
      "startArrowhead": null,
      "endArrowhead": "arrow",
      "elbowed": false
    },
    {
      "id": "guaxi-sample-db",
      "type": "ellipse",
      "x": 300,
      "y": 268,
      "width": 160,
      "height": 80,
      "angle": 0,
      "strokeColor": "#e8590c",
      "backgroundColor": "#ffe8cc",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "solid",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "roundness": { "type": 2 },
      "seed": 1007,
      "version": 1,
      "versionNonce": 1007,
      "isDeleted": false,
      "boundElements": [
        { "type": "text", "id": "guaxi-sample-db-text" }
      ],
      "updated": 1,
      "link": null,
      "locked": false
    },
    {
      "id": "guaxi-sample-db-text",
      "type": "text",
      "x": 336,
      "y": 296,
      "width": 88,
      "height": 25,
      "angle": 0,
      "strokeColor": "#e8590c",
      "backgroundColor": "transparent",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "solid",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "roundness": null,
      "seed": 1008,
      "version": 1,
      "versionNonce": 1008,
      "isDeleted": false,
      "boundElements": null,
      "updated": 1,
      "link": null,
      "locked": false,
      "text": "数据存储",
      "fontSize": 20,
      "fontFamily": 1,
      "textAlign": "center",
      "verticalAlign": "middle",
      "containerId": "guaxi-sample-db",
      "originalText": "数据存储",
      "lineHeight": 1.25,
      "baseline": 18
    },
    {
      "id": "guaxi-sample-arrow-2",
      "type": "arrow",
      "x": 380,
      "y": 216,      "width": 0,
      "height": 48,
      "angle": 0,
      "strokeColor": "#1e1e1e",
      "backgroundColor": "transparent",
      "fillStyle": "solid",
      "strokeWidth": 2,
      "strokeStyle": "solid",
      "roughness": 1,
      "opacity": 100,
      "groupIds": [],
      "frameId": null,
      "roundness": { "type": 2 },
      "seed": 1009,
      "version": 1,
      "versionNonce": 1009,
      "isDeleted": false,
      "boundElements": null,
      "updated": 1,
      "link": null,
      "locked": false,
      "points": [
        [0, 0],
        [0, 48]
      ],
      "lastCommittedPoint": null,
      "startBinding": {
        "elementId": "guaxi-sample-server",
        "focus": 0,
        "gap": 6
      },
      "endBinding": {
        "elementId": "guaxi-sample-db",
        "focus": 0,
        "gap": 6
      },
      "startArrowhead": null,
      "endArrowhead": "arrow",
      "elbowed": false
    }
  ],
  "appState": {
    "gridSize": null,
    "viewBackgroundColor": "#ffffff"
  },
  "files": {}
}
`,format:"json"}}),Ae=()=>F.list(),Ne=e=>F.get(e);globalThis.GuaxiChartPreview={parseChartFence:Se,normalizeDataset:he,buildChartOption:xe,buildLightOption:pe,CHART_TYPES:v,EXTENSION_BUILDERS:me,normalizeChartType:E,supportedChartTypes:B,isBundledRef:ve,getBundledSample:Ne,listBundledSamples:Ae}})();
