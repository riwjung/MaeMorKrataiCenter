import {useMasterData, type MasterData} from '../lib/master-data';
import type {DailyCard} from '../lib/master-data-adapter';

export type {DailyCard};

export function getDailyCard(data: MasterData, id: string): DailyCard {
  const card = data.daily.find(entry => entry.card_id === id);
  if (!card) throw new Error(`Missing daily reading: ${id}`);
  return card;
}

export function getLoveReading(card: DailyCard, key: string): string | null {
  return card.content.love_by_status[key] ?? null;
}

function SourceText({text}: {text: string}) {
  return <>{text.split(/(\*\*[^*]+\*\*)/g).map((part, index) => part.startsWith('**') && part.endsWith('**')
    ? <strong key={index}>{part.slice(2, -2)}</strong> : part)}</>;
}

export function DailyReading({card, context, allLove = false}: {card: DailyCard; context?: string; allLove?: boolean}) {
  const statuses = useMasterData().config.dailyStatuses;
  const content = card.content;
  const status = context ? statuses.find(s => s.label === context) : undefined;
  const love = status ? getLoveReading(card, status.key) : null;
  return <div className="daily-reading">
    <section><h3>ภาพรวมวันนี้</h3><p><SourceText text={content.overview}/></p></section>
    <section><h3>การงาน</h3><p><SourceText text={content.work}/></p></section>
    <section><h3>การเงิน</h3><p><SourceText text={content.money}/></p></section>
    {context && <section><h3>ความรัก · {context}</h3><p><SourceText text={love ?? 'แชทต้นทางยังไม่มีคำอ่านสำหรับสถานะนี้ของไพ่ใบนี้'}/></p></section>}
    <section><h3>ข้อความจากแม่หมอ</h3><p><SourceText text={content.rabbit_message}/></p></section>
    <div className="daily-extras"><p><strong>สีประจำวัน</strong> {content.colors}</p><p><strong>คำยืนยันกับตัวเอง</strong> {content.affirmation}</p></div>
    {allLove && <div className="reading-full"><h3>{context ? 'ความรักในสถานะอื่น' : 'ความรักตามสถานะ'}</h3><dl>{statuses.filter(s => s.label !== context).map(s => <div key={s.key}><dt>{s.label}</dt><dd>{getLoveReading(card, s.key) ?? 'ยังไม่มีคำอ่านในแชทต้นทาง'}</dd></div>)}</dl></div>}
  </div>;
}
