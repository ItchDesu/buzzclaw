import React, { useState, useEffect, useRef } from 'react';
import { QRCodeSVG } from 'qrcode.react';
import { motion, AnimatePresence } from 'framer-motion';
import {
  RefreshCcw,
  Send,
  MoreVertical,
  Paperclip,
  Smile,
  CheckCheck,
  Search,
  ArrowLeft
} from 'lucide-react';
import { cn } from '@/lib/utils';

// We'll use a data URL or simple path for the logo to avoid TS issues if it's missing type declarations
const LOGO_URL = './logo.svg';

const App: React.FC = () => {
  const [syncToken, setSyncToken] = useState<string>('');
  const [status, setStatus] = useState<'idle' | 'linking' | 'linked'>('idle');
  const [progress, setProgress] = useState(0);
  const [roomId, setRoomId] = useState<string>('');
  const [wsStatus, setWsStatus] = useState<'connecting' | 'connected' | 'error'>('connecting');
  const [messages, setMessages] = useState<Array<{ content: string; isPeer: boolean; time: string }>>([]);
  const [inputMessage, setInputMessage] = useState('');
  const [isTyping, setIsTyping] = useState(false);
  const [isOnline, setIsOnline] = useState(false);
  const chatEndRef = useRef<HTMLDivElement>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const RELAY_BASE = 'ntfy.sh';

  const scrollToBottom = () => {
    chatEndRef.current?.scrollIntoView({ behavior: "smooth" });
  };

  useEffect(() => {
    const timeout = setTimeout(scrollToBottom, 100);
    return () => clearTimeout(timeout);
  }, [messages, isTyping]);

  const generateNewToken = () => {
    setStatus('idle');
    setProgress(0);
    const newRoomId = Math.random().toString(36).substring(2, 15);
    setRoomId(newRoomId);
    setSyncToken(`buzzclaw_sync_${newRoomId}`);

    // Close previous socket if any
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }

    setWsStatus('connecting');
    const wsUrl = `wss://${RELAY_BASE}/buzzclaw_sync_${newRoomId}/ws`;
    const newSocket = new WebSocket(wsUrl);
    wsRef.current = newSocket;

    newSocket.onopen = () => {
      setWsStatus('connected');
      // Request initial status
      fetch(`https://${RELAY_BASE}/buzzclaw_sync_${newRoomId}`, {
        method: 'POST',
        body: JSON.stringify({ type: 'status_request', target: 'android', roomId: newRoomId })
      }).catch(console.error);
    };

    newSocket.onerror = () => {
      setWsStatus('error');
    };

    newSocket.onclose = () => {
      setWsStatus('error');
    };

    newSocket.onmessage = (event) => {
      try {
        const raw = JSON.parse(event.data);
        if (!raw.message) return;
        const msg = JSON.parse(raw.message);

        if (msg.roomId === newRoomId && msg.target === 'web') {
          if (msg.type === 'link_success') {
            setStatus('linking');
            setIsOnline(true);
          } else if (msg.type === 'chat_message') {
            setMessages(prev => [...prev, {
              content: msg.data.content,
              isPeer: msg.data.isPeer,
              time: msg.data.time || new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
            }]);
            setIsTyping(false);
          } else if (msg.type === 'history_response') {
            const history = msg.data.messages.map((m: any) => ({
              ...m,
              time: m.time || new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
            }));
            setMessages(history);
          } else if (msg.type === 'status_update') {
            if (msg.data.status === 'typing') {
              setIsTyping(true);
              setTimeout(() => setIsTyping(false), 3000);
            } else if (msg.data.status === 'online') {
              setIsOnline(true);
            } else if (msg.data.status === 'offline') {
              setIsOnline(false);
            }
          }
        }
      } catch (e) { }
    };
  };

  const handleSendMessage = async () => {
    if (!inputMessage.trim() || !roomId) return;

    const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    const data = { content: inputMessage, isPeer: false, time };
    const msg = {
      type: 'chat_message',
      roomId: roomId,
      target: 'android',
      data: data
    };

    try {
      await fetch(`https://${RELAY_BASE}/buzzclaw_sync_${roomId}`, {
        method: 'POST',
        body: JSON.stringify(msg)
      });
      setMessages(prev => [...prev, data]);
      setInputMessage('');
      if (textareaRef.current) {
        textareaRef.current.style.height = '40px';
      }
    } catch (e) {
      console.error('Failed to send message', e);
    }
  };

  useEffect(() => {
    generateNewToken();
  }, []);

  useEffect(() => {
    if (status === 'linking') {
      const interval = setInterval(() => {
        setProgress(prev => {
          if (prev >= 100) {
            clearInterval(interval);
            setStatus('linked');
            // Request full history once linked
            fetch(`https://${RELAY_BASE}/buzzclaw_sync_${roomId}`, {
              method: 'POST',
              body: JSON.stringify({ type: 'history_request', target: 'android', roomId: roomId })
            }).catch(console.error);
            return 100;
          }
          return prev + 5;
        });
      }, 50);
      return () => clearInterval(interval);
    }
  }, [status]);

  if (status !== 'linked') {
    return (
      <div className="min-h-screen flex flex-col items-center justify-center bg-[#0e1621] text-white font-sans p-4 relative overflow-hidden">
        {/* Abstract Background Accents */}
        <div className="absolute top-[-10%] right-[-10%] w-[40%] h-[40%] bg-[#2b5278]/10 blur-[120px] rounded-full" />
        <div className="absolute bottom-[-10%] left-[-10%] w-[40%] h-[40%] bg-[#4da3ff]/10 blur-[120px] rounded-full" />

        <motion.div
          initial={{ opacity: 0, scale: 0.95 }}
          animate={{ opacity: 1, scale: 1 }}
          className="w-full max-w-[440px] bg-[#17212b] rounded-[24px] shadow-2xl p-10 border border-white/5 text-center space-y-8 z-10"
        >
          <div className="flex flex-col items-center gap-5">
            <div className="w-24 h-24 bg-[#242f3d] rounded-[32px] flex items-center justify-center p-5 shadow-inner">
              <img src={LOGO_URL} alt="BuzzClaw" className="w-full h-full object-contain" />
            </div>
            <div className="space-y-2">
              <h1 className="text-2xl font-bold tracking-tight">Log in to BuzzClaw</h1>
              <p className="text-[#808b95] text-sm leading-relaxed max-w-[300px] mx-auto">
                Scan this QR code with your BuzzClaw mobile app to sync your workspace securely.
              </p>
            </div>
          </div>

          <div className="relative group p-5 bg-white rounded-[24px] shadow-2xl inline-block overflow-hidden mx-auto transition-transform hover:scale-[1.02]">
            {status === 'linking' && (
              <div className="absolute inset-0 z-20 bg-white/90 flex flex-col items-center justify-center p-8 text-black backdrop-blur-sm">
                <div className="relative w-full h-1.5 bg-gray-100 rounded-full mb-3">
                  <motion.div
                    className="absolute h-full bg-[#2b5278] rounded-full"
                    initial={{ width: 0 }}
                    animate={{ width: `${progress}%` }}
                  />
                </div>
                <span className="text-[10px] font-black uppercase tracking-[0.2em] text-[#2b5278]">Encrypted Sync...</span>
              </div>
            )}
            {syncToken ? (
              <QRCodeSVG
                value={syncToken}
                size={260}
                level="H"
                includeMargin={false}
                imageSettings={{
                  src: LOGO_URL,
                  height: 60,
                  width: 60,
                  excavate: true,
                }}
              />
            ) : (
              <div className="w-[260px] h-[260px] flex items-center justify-center bg-gray-50">
                <RefreshCcw className="w-8 h-8 text-[#2b5278] animate-spin" />
              </div>
            )}
          </div>

          <div className="space-y-6 pt-2">
            <div className="flex items-center justify-center gap-3">
              <div className={cn(
                "w-2.5 h-2.5 rounded-full border border-black/20 shadow-sm",
                wsStatus === 'connected' ? "bg-emerald-500" :
                  wsStatus === 'error' ? "bg-rose-500" : "bg-amber-500 animate-pulse"
              )} />
              <span className="text-[#808b95] uppercase text-[10px] font-black tracking-[0.15em]">
                {wsStatus === 'connected' ? 'Secure Relay Active' :
                  wsStatus === 'error' ? 'Handshake Failed' : 'Initializing Bridge...'}
              </span>
            </div>

            <button
              onClick={generateNewToken}
              className="w-full py-4 bg-[#2b5278] hover:bg-[#33618d] text-white font-bold rounded-2xl transition-all flex items-center justify-center gap-3 shadow-lg active:scale-95 group"
            >
              Refresh Token
              <RefreshCcw className="w-5 h-5 group-hover:rotate-180 transition-transform duration-500" />
            </button>
          </div>
        </motion.div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-screen bg-[#0e1621] text-[#f5f5f5] font-sans selection:bg-[#2b5278]/50 overflow-hidden relative">
      {/* Pattern Background Overlay */}
      <div className="absolute inset-0 opacity-[0.05] pointer-events-none bg-[url('https://telegram.org/img/t_desktop_bg.png')] bg-repeat z-0" />

      {/* Main Container - Telegram Web A Style (No Sidebar) */}
      <div className="flex-1 flex flex-col w-full max-w-[800px] mx-auto bg-[#17212b] shadow-[0_0_80px_rgba(0,0,0,0.5)] z-10 overflow-hidden relative">

        <header className="h-[64px] px-4 flex items-center justify-between shrink-0 border-b border-black/20 bg-[#17212b]/98 backdrop-blur-xl z-20">
          <div className="flex items-center gap-3">
            <button className="p-2.5 hover:bg-white/5 rounded-full text-[#808b95] transition-colors">
              <ArrowLeft className="w-6 h-6" />
            </button>
            <div className="w-11 h-11 bg-gradient-to-br from-[#4da3ff] to-[#2b5278] rounded-full flex items-center justify-center p-2 shadow-lg">
              <img src={LOGO_URL} alt="B" className="w-full h-full object-contain invert brightness-0" />
            </div>
            <div className="flex flex-col ml-1">
              <h2 className="font-bold text-[17px] tracking-tight">BuzzClaw</h2>
              <div className="h-4 flex items-center">
                {isTyping ? (
                  <span className="text-[13px] text-[#4da3ff] font-medium italic">typing...</span>
                ) : (
                  <span className={cn(
                    "text-[13px] font-medium transition-colors duration-500",
                    isOnline ? "text-[#4da3ff]" : "text-[#808b95]"
                  )}>
                    {isOnline ? 'online' : 'offline'}
                  </span>
                )}
              </div>
            </div>
          </div>
          <div className="flex items-center gap-1">
            <button className="p-2.5 hover:bg-white/5 rounded-full text-[#808b95]"><Search className="w-5 h-5" /></button>
            <button className="p-2.5 hover:bg-white/5 rounded-full text-[#808b95]"><MoreVertical className="w-5 h-5" /></button>
          </div>
        </header>

        {/* Chat Content Area */}
        <div className="flex-1 overflow-y-auto px-4 py-6 flex flex-col gap-1.5 custom-scrollbar bg-[#0e1621]/40 relative">
          {messages.length === 0 && (
            <div className="flex flex-col items-center justify-center h-full opacity-30 gap-6 select-none">
              <div className="p-8 bg-[#17212b] rounded-[40px] shadow-2xl scale-110">
                <img src={LOGO_URL} alt="BuzzClaw" className="w-24 h-24 grayscale brightness-125" />
              </div>
              <div className="text-center space-y-1">
                <p className="text-[15px] font-bold tracking-wide">Sync Established</p>
                <p className="text-[13px] text-[#808b95]">Waiting for messages from BuzzClaw...</p>
              </div>
            </div>
          )}

          <AnimatePresence initial={false}>
            {messages.map((msg, idx) => (
              <motion.div
                key={idx}
                initial={{ opacity: 0, scale: 0.98, y: 4 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                className={cn(
                  "max-w-[72%] group flex flex-col",
                  msg.isPeer ? "self-start" : "self-end"
                )}
              >
                <div className={cn(
                  "px-3.5 py-1.5 rounded-[18px] text-[15.5px] relative shadow-lg min-w-[64px] transition-all",
                  msg.isPeer
                    ? "bg-[#182533] rounded-bl-[4px] text-white border border-white/5"
                    : "bg-[#2b5278] rounded-br-[4px] text-white"
                )}>
                  <div className="whitespace-pre-wrap leading-relaxed py-0.5">{msg.content}</div>
                  <div className="flex items-center justify-end gap-1.5 mt-1 shrink-0 -mr-1 select-none">
                    <span className="text-[10px] text-white/40 font-bold uppercase tracking-wider">{msg.time}</span>
                    {!msg.isPeer && <CheckCheck className="w-3.5 h-3.5 text-[#4da3ff]" />}
                  </div>
                </div>
              </motion.div>
            ))}
          </AnimatePresence>
          <div ref={chatEndRef} className="h-6" />
        </div>

        {/* Floating Input Bar - Telegram A Style */}
        <footer className="p-4 bg-transparent shrink-0 z-20">
          <div className="max-w-[720px] mx-auto flex items-end gap-2 p-2 rounded-[24px] bg-[#17212b] shadow-2xl border border-white/5">
            <button className="p-3 hover:bg-white/5 rounded-full text-[#808b95] shrink-0 transition-colors">
              <Paperclip className="w-6 h-6" />
            </button>
            <div className="flex-1 relative mb-1.5">
              <textarea
                ref={textareaRef}
                rows={1}
                value={inputMessage}
                onChange={(e) => {
                  setInputMessage(e.target.value);
                  // Reset height to recalculate
                  e.target.style.height = 'auto';
                  e.target.style.height = `${e.target.scrollHeight}px`;
                }}
                onKeyDown={(e) => (e.key === 'Enter' && !e.shiftKey) && (e.preventDefault(), handleSendMessage())}
                placeholder="Message"
                className="w-full bg-transparent border-none outline-none resize-none py-2 px-2 text-[16px] placeholder:text-[#808b95] max-h-[360px] overflow-y-auto scrollbar-hide flex items-center leading-normal"
                style={{ height: '40px' }}
              />
            </div>
            <button className="p-3 hover:bg-white/5 rounded-full text-[#808b95] shrink-0 transition-colors">
              <Smile className="w-6 h-6" />
            </button>
            <button
              onClick={handleSendMessage}
              disabled={!inputMessage.trim()}
              className={cn(
                "w-12 h-12 rounded-full transition-all shrink-0 flex items-center justify-center shadow-lg",
                inputMessage.trim() ? "bg-[#2b5278] text-white scale-100 active:scale-90" : "bg-[#242f3d]/50 text-[#808b95] opacity-50"
              )}
            >
              <Send className="w-5 h-5 translate-x-[1px]" />
            </button>
          </div>
          <div className="h-2" />
        </footer >
      </div>
      <div className="h-6" />
    </div>
  );
};

export default App;
