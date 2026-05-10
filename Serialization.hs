module Serialization
  ( Timeline
  , loadTimeline
  , saveTimeline
  , calculateGuilt
  , scoreInnocence
  , toJSON
  ) where

import qualified Data.Aeson as A
import Data.Aeson ((.:))
import qualified Data.ByteString.Lazy as BL
import qualified Data.ByteString.Lazy.Char8 as C8
import Data.Char (toLower)
import Data.IORef (IORef, newIORef, readIORef, writeIORef)
import Data.List (isInfixOf, nub)
import System.IO.Unsafe (unsafePerformIO)
import Timeline (Alibi(..), Evidence(..), Suspect, TimeRange(..))

data PhaseRecord = PhaseRecord
  { prType        :: String
  , prTypeCode    :: Int
  , prTimestamp   :: Int
  , prReliability :: Int
  , prDescription :: String
  , prCrc         :: Int
  , prCrcValid    :: Bool
  }

data PhaseOutput = PhaseOutput
  { outputRecords :: [PhaseRecord]
  }

instance A.FromJSON PhaseRecord where
  parseJSON = A.withObject "PhaseRecord" $ \o ->
    PhaseRecord
      <$> o .: "type"
      <*> o .: "type_code"
      <*> o .: "timestamp"
      <*> o .: "reliability"
      <*> o .: "description"
      <*> o .: "crc"
      <*> o .: "crc_valid"

instance A.FromJSON PhaseOutput where
  parseJSON = A.withObject "PhaseOutput" $ \o ->
    PhaseOutput <$> o .: "records"

data Timeline = Timeline
  { timelineAlibis :: [Alibi]
  , timelineEvidence :: [Evidence]
  } deriving (Eq, Show)

instance A.ToJSON Timeline where
  toJSON (Timeline alibis evidence) =
    A.object
      [ "alibis" A..= map alibiRecord alibis
      , "evidence" A..= map evidenceRecord evidence
      ]
    where
      alibiRecord (Alibi person rng witnesses confidence) =
        A.object
          [ "person" A..= person
          , "timeRange" A..= A.object ["start" A..= startTime rng, "end" A..= endTime rng]
          , "witnesses" A..= witnesses
          , "confidence" A..= confidence
          ]
      evidenceRecord (Evidence description timestamp reliability suspect) =
        A.object
          [ "description" A..= description
          , "timestamp" A..= timestamp
          , "reliability" A..= reliability
          , "suspectInvolved" A..= suspect
          ]

timelineRef :: IORef Timeline
timelineRef = unsafePerformIO (newIORef (Timeline [] []))

loadTimeline :: FilePath -> IO Timeline
loadTimeline path = do
  contents <- BL.readFile path
  case A.eitherDecode contents of
    Left err -> ioError (userError err)
    Right output -> do
      let tl = timelineFromOutput output
      writeIORef timelineRef tl
      pure tl

saveTimeline :: FilePath -> Timeline -> IO ()
saveTimeline path tl = BL.writeFile path (A.encode tl)

toJSON :: Timeline -> String
toJSON = C8.unpack . A.encode

scoreInnocence :: Suspect -> Timeline -> Float
scoreInnocence suspect (Timeline alibis evidence) =
  let alibiScores = [alibiConfidence a | a <- alibis, alibiPerson a == suspect]
      bestAlibi = maximum (0 : alibiScores)
      conflicts = [evidenceReliability e | e <- evidence, evidenceSuspectInvolved e == suspect]
      conflictImpact = maximum (0 : conflicts)
      raw = bestAlibi * 0.7 + (1 - conflictImpact) * 0.3
   in realToFrac (max 0 (min 1 raw))

calculateGuilt :: Suspect -> Float
calculateGuilt suspect = 1 - scoreInnocence suspect (unsafePerformIO (readIORef timelineRef))

timelineFromOutput :: PhaseOutput -> Timeline
timelineFromOutput (PhaseOutput records) =
  Timeline
    (map (recordToAlibi suspects) (filter ((== 5) . prTypeCode) records))
    (map (recordToEvidence suspects) (filter ((/= 5) . prTypeCode) records))
  where
    suspects = map prDescription (filter (matchType "suspect") records)
    matchType target record = map toLower (prType record) == target

recordToAlibi :: [String] -> PhaseRecord -> Alibi
recordToAlibi names rec =
  Alibi person (makeRange (prTimestamp rec)) (filter (/= person) wits) conf
  where
    desc = prDescription rec
    person = detectName desc names
    wits = nub (namesInDescription names desc)
    conf = fromIntegral (prReliability rec) / 255

recordToEvidence :: [String] -> PhaseRecord -> Evidence
recordToEvidence names rec =
  let desc = prDescription rec
      conf = fromIntegral (prReliability rec) / 255
   in Evidence desc (prTimestamp rec) conf (detectName desc names)

namesInDescription :: [String] -> String -> [String]
namesInDescription names desc =
  let lowerDesc = map toLower desc
   in nub [name | name <- names, map toLower name `isInfixOf` lowerDesc]

detectName :: String -> [String] -> String
detectName desc names = case namesInDescription names desc of
  (name : _) -> name
  [] -> desc

makeRange :: Int -> TimeRange
makeRange timestamp =
  let start = max 0 (timestamp - 15)
   in TimeRange start (timestamp + 15)
