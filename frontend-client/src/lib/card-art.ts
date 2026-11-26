import {getMasterData} from './master-data';

type CardArt = {id:string;art?:string|null};
export function cardArtSize(card:CardArt) {
 const art=getMasterData().art.find(item=>item.id===card.id&&item.art===card.art);
 return art?{...art,width:art.renderedWidth,height:art.renderedHeight}:undefined;
}
export function cardAspectRatio(card:CardArt) {
 const art=cardArtSize(card);
 return art?`${art.width} / ${art.height}`:'1632 / 2624';
}
