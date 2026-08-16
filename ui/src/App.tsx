import { useState, useEffect, useRef } from 'react'
import { Search, FolderGit2, ShieldAlert, List, Settings, Minus, Square, X, Play, Loader2, Database, Network } from 'lucide-react'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import * as d3 from 'd3'
import './index.css'

function App() {
  const [activeTab, setActiveTab] = useState('index')
  const [activeGraph, setActiveGraph] = useState('godnodes')
  const [indexedProject, setIndexedProject] = useState<string | null>(null)
  const [isIndexing, setIsIndexing] = useState(false)
  const [graphData, setGraphData] = useState<{nodes: any[], links: any[]} | null>(null)
  
  // Chat State
  const [chatMessages, setChatMessages] = useState<{role: 'user'|'sentinel', text: string}[]>([
    { role: 'sentinel', text: 'System initialized in Air-Gapped mode. Awaiting codebase index.' }
  ])
  const [chatInput, setChatInput] = useState('')
  const [isTyping, setIsTyping] = useState(false)
  const chatEndRef = useRef<HTMLDivElement>(null)
  
  // Search State
  const [searchInput, setSearchInput] = useState('')
  const [searchResults, setSearchResults] = useState<any[]>([])
  const [isSearching, setIsSearching] = useState(false)
  
  const svgRef = useRef<SVGSVGElement>(null)
  
  const appWindow = getCurrentWindow();
  
  const handleMinimize = () => appWindow.minimize()
  const handleMaximize = () => appWindow.toggleMaximize()
  const handleClose = () => appWindow.close()

  const handleOpenDirectory = async () => {
    try {
      // Call our Rust Tauri command to open the native folder picker
      const path = await invoke<string | null>('cmd_open_folder')
      if (path) {
        setIsIndexing(true)
        setIndexedProject(path)
        
        // Call the backend to run the fast xxhash index pipeline
        await invoke('cmd_index_folder', { dir: path })
        
        // Wait briefly for the indexer background process, then load the true AST graph
        setTimeout(async () => {
          try {
            const graphJson = await invoke<string>('cmd_get_graph')
            const data = JSON.parse(graphJson)
            setGraphData(data)
          } catch (e) {
            console.error("Failed to load graph", e)
          }
          setIsIndexing(false)
        }, 1500)
      }
    } catch (e) {
      console.error(e)
      setIsIndexing(false)
    }
  }

  // D3 Graph Rendering
  useEffect(() => {
    if ((activeTab === 'explorer' || activeTab === 'graphs') && graphData && svgRef.current) {
      const svg = d3.select(svgRef.current)
      svg.selectAll("*").remove()
      
      const width = svgRef.current.clientWidth
      const height = svgRef.current.clientHeight
      
      const simulation = d3.forceSimulation(graphData.nodes)
        .force("link", d3.forceLink(graphData.links).id((d: any) => d.id).distance(100))
        .force("charge", d3.forceManyBody().strength(-300))
        .force("center", d3.forceCenter(width / 2, height / 2))

      const colorScale = d3.scaleOrdinal(d3.schemeCategory10)

      const link = svg.append("g")
        .attr("stroke", "var(--border-focus)")
        .attr("stroke-opacity", 0.6)
        .selectAll("line")
        .data(graphData.links)
        .join("line")
        .attr("stroke-width", (d: any) => Math.sqrt(d.value))

      const node = svg.append("g")
        .attr("stroke", "var(--bg-dark)")
        .attr("stroke-width", 1.5)
        .selectAll("circle")
        .data(graphData.nodes)
        .join("circle")
        .attr("r", (d: any) => activeGraph === 'godnodes' ? d.radius : 10)
        .attr("fill", (d: any) => activeGraph === 'communities' ? colorScale(d.group) as string : "var(--accent)")

      node.append("title").text((d: any) => `${d.name} (${d.kind})\nCommunity: ${d.group}`)

      simulation.on("tick", () => {
        link
          .attr("x1", (d: any) => d.source.x)
          .attr("y1", (d: any) => d.source.y)
          .attr("x2", (d: any) => d.target.x)
          .attr("y2", (d: any) => d.target.y)
        node
          .attr("cx", (d: any) => d.x)
          .attr("cy", (d: any) => d.y)
      })
    }
  }, [activeTab, activeGraph, graphData])

  // Scroll to bottom of chat
  useEffect(() => {
    chatEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [chatMessages])

  const handleChatSubmit = async (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && chatInput.trim() !== '') {
      const q = chatInput.trim()
      setChatInput('')
      setChatMessages(prev => [...prev, { role: 'user', text: q }])
      setIsTyping(true)
      
      try {
        const response = await invoke<string>('cmd_ask', { question: q })
        setChatMessages(prev => [...prev, { role: 'sentinel', text: response }])
      } catch (err) {
        setChatMessages(prev => [...prev, { role: 'sentinel', text: `[Error: ${err}]` }])
      }
      setIsTyping(false)
    }
  }

  const handleSearchSubmit = async (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && searchInput.trim() !== '') {
      const q = searchInput.trim()
      setIsSearching(true)
      setActiveTab('search')
      
      try {
        const resultsJson = await invoke<string>('cmd_search', { q, limit: 10 })
        setSearchResults(JSON.parse(resultsJson))
      } catch (err) {
        console.error("Search error:", err)
      }
      setIsSearching(false)
    }
  }

  return (
    <>
      
      <nav id="navbar">
        <div className="container" style={{ display: 'flex', width: '100%', alignItems: 'center' }}>
          <div className="nav-logo" style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '18px', fontWeight: 600, color: 'var(--text)' }}>
            <Network size={20} color="var(--accent)" />
            Needle
          </div>
          
          <div className="nav-sep" style={{ margin: '0 24px' }}></div>
          
          <div className="nav-links">
            <div className={`nav-link ${activeTab === 'search' ? 'active' : ''}`} onClick={() => setActiveTab('search')}>
              <Search size={14} /> Search
            </div>
            <div className={`nav-link ${activeTab === 'index' ? 'active' : ''}`} onClick={() => setActiveTab('index')}>
              <FolderGit2 size={14} /> Index
            </div>
            <div className={`nav-link ${activeTab === 'graphs' ? 'active' : ''}`} onClick={() => setActiveTab('graphs')}>
              <Network size={14} /> Graph
            </div>
            <div className={`nav-link ${activeTab === 'auditor' ? 'active' : ''}`} onClick={() => setActiveTab('auditor')} style={{ color: 'var(--success)' }}>
              <ShieldAlert size={14} /> Sentinel
            </div>
            
            <div className="nav-link" style={{ position: 'relative' }}>
              More ▾
            </div>
          </div>

          <div className="window-controls" style={{ marginLeft: 'auto', display: 'flex', gap: '8px' }}>
            <button className="icon-btn" onClick={handleMinimize}><Minus size={16} /></button>
            <button className="icon-btn" onClick={handleMaximize}><Square size={14} /></button>
            <button className="icon-btn" onClick={handleClose}><X size={16} /></button>
          </div>
        </div>
      </nav>

      <main className="container" style={{ paddingTop: '40px', paddingBottom: '80px', height: 'calc(100vh - 56px)', overflowY: 'auto' }}>
        <div className="page active">

          
          {/* Index Tab */}
          {activeTab === 'index' && (
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--text-secondary)' }}>
              {!indexedProject && !isIndexing ? (
                <>
                  <FolderGit2 size={48} style={{ opacity: 0.5, marginBottom: '16px' }} />
                  <p style={{ marginBottom: '16px' }}>No project indexed. Connect a local folder to begin.</p>
                  <button onClick={handleOpenDirectory} style={{ 
                    padding: '8px 16px', 
                    background: 'var(--accent)', 
                    color: '#fff', 
                    border: 'none', 
                    borderRadius: '4px',
                    cursor: 'pointer',
                    fontWeight: 500
                  }}>
                    Open Directory
                  </button>
                </>
              ) : isIndexing ? (
                <>
                  <Loader2 className="lucide-spin" size={48} style={{ opacity: 0.5, marginBottom: '16px', color: 'var(--accent)' }} />
                  <p>Parsing Abstract Syntax Trees...</p>
                </>
              ) : (
                <>
                  <Database size={48} style={{ opacity: 0.5, marginBottom: '16px', color: 'var(--success)' }} />
                  <p>Index built successfully for <code style={{background:'transparent', padding:0}}>{indexedProject}</code>.</p>
                  <button onClick={handleOpenDirectory} style={{ 
                    marginTop: '16px',
                    padding: '8px 16px', 
                    background: 'transparent', 
                    color: 'var(--accent)', 
                    border: '1px solid var(--border-focus)', 
                    borderRadius: '4px',
                    cursor: 'pointer',
                  }}>
                    Change Directory
                  </button>
                </>
              )}
            </div>
          )}

          {/* Search Tab */}
          {activeTab === 'search' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px', maxWidth: '800px', margin: '0 auto', width: '100%' }}>
              <div style={{ display: 'flex', gap: '8px', width: '100%' }}>
                <input 
                  type="text" 
                  placeholder="Query codebase via HNSW + BM25 (Ctrl+K)..." 
                  value={searchInput}
                  onChange={e => setSearchInput(e.target.value)}
                  onKeyDown={handleSearchSubmit}
                  style={{
                    flexGrow: 1,
                    padding: '12px 16px',
                    background: 'var(--surface)',
                    border: '1px solid var(--border)',
                    borderRadius: 'var(--radius)',
                    color: 'var(--text)',
                    fontSize: '15px'
                  }}
                />
                <button 
                  onClick={() => handleSearchSubmit({ key: 'Enter' } as any)}
                  style={{
                    padding: '0 24px',
                    background: 'var(--accent)',
                    color: '#fff',
                    border: 'none',
                    borderRadius: 'var(--radius)',
                    cursor: 'pointer',
                    fontWeight: 600
                  }}
                >
                  Search
                </button>
              </div>

              {isSearching && (
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', color: 'var(--text-2)' }}>
                  <Loader2 className="lucide-spin" size={16} /> Querying Vector Index...
                </div>
              )}
              
              {!isSearching && searchResults.length > 0 && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
                  {searchResults.map((res, i) => (
                    <div key={i} style={{ 
                      background: 'var(--surface)', 
                      border: '1px solid var(--border)', 
                      borderRadius: 'var(--radius)',
                      overflow: 'hidden'
                    }}>
                      <div style={{ 
                        padding: '8px 12px', 
                        borderBottom: '1px solid var(--border)',
                        display: 'flex',
                        justifyContent: 'space-between',
                        alignItems: 'center',
                        fontSize: '12px',
                        color: 'var(--text-2)',
                        background: 'var(--bg3)'
                      }}>
                        <span style={{ color: 'var(--accent)' }}>{res.file_path}:{res.line_start}-{res.line_end}</span>
                        <span>Score: {res.score.toFixed(2)} | {res.language}</span>
                      </div>
                      <pre style={{ 
                        margin: 0, 
                        padding: '16px', 
                        fontSize: '13px', 
                        fontFamily: 'var(--mono)',
                        overflowX: 'auto',
                        color: 'var(--text)',
                        lineHeight: '1.5'
                      }}>
                        <code>{res.content.trim()}</code>
                      </pre>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Graph Tab */}
          {activeTab === 'graphs' && (
            <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
              {!indexedProject ? (
                <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--text-2)' }}>
                  <Network size={48} style={{ opacity: 0.5, marginBottom: '16px' }} />
                  <p>Index a project first to visualize its topology.</p>
                </div>
              ) : (
                <>
                  <div style={{ display: 'flex', gap: '8px', marginBottom: '16px' }}>
                    <button 
                      className={`tab-btn ${activeGraph === 'godnodes' ? 'active' : ''}`}
                      onClick={() => setActiveGraph('godnodes')}
                    >God Nodes</button>
                    <button 
                      className={`tab-btn ${activeGraph === 'communities' ? 'active' : ''}`}
                      onClick={() => setActiveGraph('communities')}
                    >Communities</button>
                    <button 
                      className={`tab-btn ${activeGraph === 'surprises' ? 'active' : ''}`}
                      onClick={() => setActiveGraph('surprises')}
                    >Surprises</button>
                  </div>
                  <div style={{ flexGrow: 1, border: '1px solid var(--border)', borderRadius: 'var(--radius)', background: 'var(--code-bg)', position: 'relative' }}>
                    <svg ref={svgRef} style={{ width: '100%', height: '100%' }}></svg>
                  </div>
                </>
              )}
            </div>
          )}

          {/* Sentinel Chat Tab */}
          {activeTab === 'auditor' && (
            <div style={{ display: 'flex', flexDirection: 'column', height: '100%', maxWidth: '800px', margin: '0 auto', width: '100%' }}>
              <div style={{ 
                flexGrow: 1, 
                overflowY: 'auto', 
                padding: '16px', 
                display: 'flex', 
                flexDirection: 'column', 
                gap: '16px',
                background: 'var(--surface)',
                border: '1px solid var(--border)',
                borderRadius: 'var(--radius)',
                marginBottom: '16px'
              }}>
                {chatMessages.map((msg, i) => (
                  <div key={i} style={{ 
                    background: msg.role === 'sentinel' ? 'var(--bg3)' : 'var(--accent-subtle)', 
                    border: msg.role === 'sentinel' ? '1px solid var(--border2)' : '1px solid var(--accent-glow)', 
                    padding: '16px', 
                    borderRadius: 'var(--radius)',
                    fontSize: '14px',
                    lineHeight: '1.6',
                    color: msg.role === 'sentinel' ? 'var(--text)' : 'var(--text)',
                    alignSelf: msg.role === 'sentinel' ? 'flex-start' : 'flex-end',
                    maxWidth: '85%'
                  }}>
                    {msg.role === 'sentinel' && <div style={{ color: 'var(--success)', fontWeight: 'bold', marginBottom: '4px', fontSize: '12px', textTransform: 'uppercase' }}>Sentinel AI</div>}
                    {msg.text}
                  </div>
                ))}
                
                {isTyping && (
                  <div style={{ padding: '16px', fontSize: '14px', color: 'var(--text-2)' }}>
                    <Loader2 className="lucide-spin" size={16} style={{ display: 'inline', marginRight: '8px' }} />
                    Querying local Ollama daemon...
                  </div>
                )}
                <div ref={chatEndRef} />
              </div>
              
              <div style={{ display: 'flex', gap: '8px' }}>
                <input 
                  type="text" 
                  value={chatInput}
                  onChange={e => setChatInput(e.target.value)}
                  onKeyDown={handleChatSubmit}
                  disabled={isTyping}
                  placeholder="Ask the local Ollama LLM about the codebase..." 
                  style={{ 
                    flexGrow: 1,
                    padding: '16px',
                    background: 'var(--surface)',
                    border: '1px solid var(--border)',
                    borderRadius: 'var(--radius)',
                    color: 'var(--text)',
                    fontSize: '15px'
                  }} 
                />
              </div>
            </div>
          )}

        </div>
      </main>
    </>
  )
}
export default App
