declare const __APP_EDITION__: string;
declare const __APP_NAME__: string;

/** 'alpha' | 'stable' */
export const APP_EDITION: string = __APP_EDITION__;
/** NOVA */
export const APP_NAME: string = __APP_NAME__;
export const APP_SUBTITLE = '能力框架 Claude Code';
export const APP_SLOGAN = '人类创造过去，AI 创造未来';
export const IS_ALPHA = APP_EDITION === 'alpha';
