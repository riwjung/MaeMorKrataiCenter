// Bundled reading taxonomy (categories, situations, suggested questions, positions, 3-card love labels).
// It is the fallback when the master-data API is not configured or fails (src/lib/master-data.ts).
// Import-free so `node --test` can load it (src/lib/master-data-adapter.test.mjs).

export const contexts = ['คนโสด', 'แอบชอบ', 'คุยกัน', 'ซับซ้อน', 'มีแฟน', 'เลิกกัน'] as const;
export type Category = 'love' | 'work' | 'money' | 'life';
export const categories: {id: Category; label: string; icon: string; hint: string}[] = [
  {id: 'love', label: 'ความรัก', icon: '♡', hint: 'ความรู้สึกและความสัมพันธ์'},
  {id: 'work', label: 'การงาน', icon: '✧', hint: 'เส้นทางที่กำลังเติบโต'},
  {id: 'money', label: 'การเงิน', icon: '☾', hint: 'ความมั่นคงและการวางแผน'},
  {id: 'life', label: 'ภาพรวมชีวิต', icon: '✺', hint: 'กลับมาฟังเสียงข้างใน'},
];
export const situations: Record<Category, readonly string[]> = {
  love: contexts,
  work: ['งานปัจจุบัน', 'มองหางานใหม่', 'ธุรกิจของตัวเอง'],
  money: ['รายรับและรายจ่าย', 'วางแผนเก็บเงิน', 'เป้าหมายระยะยาว'],
  life: ['กำลังเปลี่ยนแปลง', 'มีเรื่องต้องตัดสินใจ', 'อยากทบทวนตัวเอง'],
};
export const suggestions: Record<Category, readonly string[]> = {
  love: ['ความสัมพันธ์นี้กำลังพาฉันไปทางไหน?', 'มีอะไรในใจที่ฉันควรทำความเข้าใจ?', 'ฉันควรดูแลหัวใจตัวเองอย่างไร?'],
  work: ['ฉันอยากเห็นอะไรเปลี่ยนไปในเส้นทางการทำงาน?', 'อะไรทำให้ฉันรู้สึกติดขัดกับงานตอนนี้?', 'ฉันอยากเติบโตในทิศทางไหน?'],
  money: ['ฉันอยากปรับนิสัยการใช้เงินเรื่องไหน?', 'ความมั่นคงมีความหมายอย่างไรสำหรับฉัน?', 'เป้าหมายทางการเงินที่สำคัญตอนนี้คืออะไร?'],
  life: ['ช่วงนี้ฉันควรกลับมาฟังตัวเองเรื่องอะไร?', 'มีอะไรที่ฉันพร้อมจะปล่อยวาง?', 'ฉันอยากให้ชีวิตค่อย ๆ เปลี่ยนไปอย่างไร?'],
};
export const positions = ['อดีต', 'ปัจจุบัน', 'อนาคต'] as const;
